// Copyright (c) ZeroC, Inc.

mod jump_definition;

use async_recursion::async_recursion;
use jump_definition::get_definition_span;
use slicec::compilation_state::CompilationState;
use slicec::slice_options::SliceOptions;
use slicec_ext::diagnostic_ext::DiagnosticExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
pub mod slicec_ext;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        root_uri: Arc::new(Mutex::new(None)),
        client,
        documents: Arc::new(Mutex::new(HashMap::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[derive(Debug)]
struct Backend {
    client: Client,
    root_uri: Arc<Mutex<Option<Url>>>,
    documents: Arc<Mutex<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        *self.root_uri.lock().await = params.root_uri;
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL), // or TextDocumentSyncKind::INCREMENTAL
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "\n".to_string(),
                        " ".to_string(),
                        "{".to_string(),
                    ]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Slice language server initialized!")
            .await;

        if let Some(uri) = self.root_uri.lock().await.clone() {
            match self.find_slice_files(uri).await {
                Ok(_) => {}
                Err(e) => {
                    self.client
                        .log_message(MessageType::ERROR, format!("error: {}", e))
                        .await;
                }
            }
        }
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        // Clear the documents cache
        self.documents.lock().await.clear();
        Ok(())
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let (state, _) = self.compile_slice_files().await;

        if let Some(found_location) = get_definition_span(state, uri, position) {
            let start = Position {
                line: (found_location.start.row - 1) as u32,
                character: (found_location.start.col - 1) as u32,
            };

            let end = Position {
                line: (found_location.end.row - 1) as u32,
                character: (found_location.end.col - 1) as u32,
            };

            Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: Url::from_file_path(found_location.file).unwrap(),
                range: Range::new(start, end),
            })))
        } else {
            Ok(None)
        }
    }

    async fn completion(
        &self,
        _params: CompletionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<CompletionResponse>> {
        // Add more conditions here based on context, structure, and user input

        Ok(None)
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        // Save the document in the documents cache
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        self.documents
            .lock()
            .await
            .insert(uri.clone(), text.clone());

        // Compile all files and get the diagnostics for the opened file
        let diagnostics = self.get_diagnostics_for_uri(&uri).await;

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();

        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;

        // Compile all files and get the diagnostics for the saved file
        let diagnostics = self.get_diagnostics_for_uri(&uri).await;

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let changes = params.content_changes;

        let mut documents = self.documents.lock().await;
        let doc = documents.get_mut(&uri).expect("Document not found!");

        for change in changes {
            *doc = change.text;
        }
    }
}

impl Backend {
    async fn get_diagnostics_for_uri(&self, uri: &Url) -> Vec<Diagnostic> {
        let (state, options) = self.compile_slice_files().await;
        state
            .into_diagnostics(&options)
            .iter()
            .filter(|d| d.span().is_some_and(|s| s.file == uri.path())) // Only show diagnostics for the saved file
            .filter_map(|d| d.try_into_lsp_diagnostic())
            .collect::<Vec<_>>()
    }

    async fn compile_slice_files(&self) -> (CompilationState, SliceOptions) {
        let sources = self
            .documents
            .lock()
            .await
            .keys()
            .filter_map(|k| Some(k.to_file_path().ok()?.to_str()?.to_owned()))
            .collect::<Vec<_>>();
        let options = SliceOptions {
            sources,
            ..Default::default()
        };
        (
            slicec::compile_from_options(&options, |_| {}, |_| {}),
            options,
        )
    }

    async fn find_slice_files(&self, dir: Url) -> tokio::io::Result<()> {
        let path = dir.to_file_path().map_err(|_| {
            tokio::io::Error::new(tokio::io::ErrorKind::InvalidInput, "Invalid URL")
        })?;

        self.find_slice_files_recursive(path).await
    }

    #[async_recursion]
    async fn find_slice_files_recursive(&self, dir: std::path::PathBuf) -> tokio::io::Result<()> {
        let mut entries = tokio::fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // If it's a directory, recurse into it
                self.find_slice_files_recursive(path).await?;
            } else if path.is_file() && path.extension().unwrap_or_default() == "slice" {
                // If it's a slice file, process it
                let text = tokio::fs::read_to_string(&path).await?;

                match Url::from_file_path(path.clone()) {
                    Ok(url) => {
                        let mut documents = self.documents.lock().await;
                        documents.insert(url, text);
                    }
                    Err(_) => {
                        eprintln!("Failed to convert file path to URL: {:?}", path);
                        return Err(tokio::io::Error::new(
                            tokio::io::ErrorKind::InvalidData,
                            "Invalid file path",
                        ));
                    }
                };
            }
        }

        Ok(())
    }
}
