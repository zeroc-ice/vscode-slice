// Copyright (c) ZeroC, Inc.

use hover::get_hover_info;
use jump_definition::get_definition_span;
use slicec::{compilation_state::CompilationState, slice_options::SliceOptions};
use slicec_ext::diagnostic_ext::DiagnosticExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::{lsp_types::*, Client, LanguageServer, LspService, Server};

mod hover;
mod jump_definition;
pub mod slicec_ext;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        root_uri: Arc::new(Mutex::new(None)),
        client,
        compilation_state: Arc::new(Mutex::new(CompilationState::create())),
        compilation_options: Arc::new(Mutex::new(SliceOptions::default())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[derive(Debug)]
struct Backend {
    client: Client,
    root_uri: Arc<Mutex<Option<Url>>>,
    compilation_state: Arc<Mutex<CompilationState>>,
    compilation_options: Arc<Mutex<SliceOptions>>,
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
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Compile all files to get the initial compilation state
        let (initial_state, options) = self.compile_slice_files().await;
        let mut compilation_state_guard = self.compilation_state.lock().await;
        *compilation_state_guard = initial_state;
        let mut compilation_options_guard = self.compilation_options.lock().await;
        *compilation_options_guard = options;

        self.client
            .log_message(MessageType::INFO, "Slice language server initialized!")
            .await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let state = self.compilation_state.lock().await;

        if let Some(location) = get_definition_span(&state, uri, position) {
            let start = Position {
                line: (location.start.row - 1) as u32,
                character: (location.start.col - 1) as u32,
            };

            let end = Position {
                line: (location.end.row - 1) as u32,
                character: (location.end.col - 1) as u32,
            };

            Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: Url::from_file_path(location.file).unwrap(),
                range: Range::new(start, end),
            })))
        } else {
            Ok(None)
        }
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let state = self.compilation_state.lock().await;
        if let Some(info) = get_hover_info(&state, uri, position) {
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(info)),
                range: None,
            }))
        } else {
            Ok(None)
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file opened!")
            .await;
        self.handle_file_change(params.text_document.uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;
        self.handle_file_change(params.text_document.uri).await;
    }
}

impl Backend {
    async fn handle_file_change(&self, uri: Url) {
        let (updated_state, options) = self.compile_slice_files().await;
        let mut compilation_state_guard = self.compilation_state.lock().await;
        *compilation_state_guard = updated_state;

        let mut compilation_options_guard = self.compilation_options.lock().await;
        *compilation_options_guard = options;

        let diagnostics = self
            .get_diagnostics_for_uri(
                &uri,
                &mut compilation_state_guard,
                &compilation_options_guard,
            )
            .await;

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    // This will consume all of the diagnostics in the compilation state and return them as LSP diagnostics.
    // If you need the diagnostics again you will need to recompile.
    async fn get_diagnostics_for_uri(
        &self,
        uri: &Url,
        compilation_state: &mut CompilationState,
        options: &SliceOptions,
    ) -> Vec<Diagnostic> {
        let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
            &compilation_state.ast,
            &compilation_state.files,
            options,
        );

        diagnostics
            .iter()
            .filter(|d| d.span().is_some_and(|s| s.file == uri.path())) // Only show diagnostics for the saved file
            .filter_map(|d| d.try_into_lsp_diagnostic())
            .collect::<Vec<_>>()
    }

    async fn compile_slice_files(&self) -> (CompilationState, SliceOptions) {
        let options = SliceOptions {
            references: vec![self
                .root_uri
                .lock()
                .await
                .as_ref()
                .unwrap()
                .to_file_path()
                .unwrap()
                .display()
                .to_string()],
            ..Default::default()
        };
        (
            slicec::compile_from_options(&options, |_| {}, |_| {}),
            options,
        )
    }
}
