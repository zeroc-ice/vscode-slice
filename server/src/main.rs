use async_recursion::async_recursion;
use slicec::compile_from_strings;
use slicec::slice_file::SliceFile;
use slicec::slice_options::SliceOptions;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
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
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;

        // Fill the files with all .slice files in the workspace
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
        Ok(())
    }

    async fn completion(
        &self,
        params: CompletionParams,
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

        // Compile the file and get the diagnostics
        // Get the content of every file in the documents cache
        let diagnostics = self
            .compile_slice_file()
            .await
            .iter()
            .filter(|d| {
                d.span().is_some_and(|d| {
                    Url::from_file_path(&d.file).is_ok_and(|url| url.to_string() == uri.to_string())
                })
            }) // Only show diagnostics for the saved file
            .filter_map(|d| slicec_diagnostic_to_diagnostic(d))
            .collect::<Vec<_>>();

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        // Remove the document from the documents cache
        let uri = params.text_document.uri.clone();
        self.documents.lock().await.remove(&uri);
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        self.client
            .log_message(MessageType::INFO, "file saved!")
            .await;

        // Compile the file and get the diagnostics
        // Get the content of every file in the documents cache
        let foo = self.compile_slice_file().await;
        self.client
            .log_message(
                MessageType::INFO,
                format!(
                    "span: {:?}",
                    foo.iter().map(|d| d.span()).collect::<Vec<_>>()
                ),
            )
            .await;

        // Collect the diagnostics file names to log
        let file_names = foo
            .iter()
            .map(|f| Url::from_file_path(f.span().unwrap().file.clone()))
            .collect::<Vec<_>>();

        self.client
            .log_message(MessageType::INFO, format!("MY files: {:?}", file_names))
            .await;

        self.client
            .log_message(
                MessageType::INFO,
                format!("soruce name : {:?}", uri.to_string()),
            )
            .await;

        let diagnostics = foo
            .iter()
            .filter(|d| {
                d.span().is_some_and(|d| {
                    Url::from_file_path(&d.file).is_ok_and(|url| url.to_string() == uri.to_string())
                })
            }) // Only show diagnostics for the saved file
            .filter_map(|d| slicec_diagnostic_to_diagnostic(d))
            .collect::<Vec<_>>();

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
    async fn compile_slice_file(&self) -> Vec<slicec::diagnostics::Diagnostic> {
        let sources = self
            .documents
            .lock()
            .await
            .keys()
            .filter_map(|k| Some(k.to_file_path().ok()?.to_str()?.to_owned()))
            .collect::<Vec<_>>();
        self.client
            .log_message(MessageType::INFO, format!("sources: {:?}", sources))
            .await;
        let options = SliceOptions {
            sources,
            ..Default::default()
        };
        let state = slicec::compile_from_options(&options, |_| {}, |_| {});
        let diagnostics = state.into_diagnostics(&options);
        diagnostics
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

fn slicec_diagnostic_to_diagnostic(
    diagnostic: &slicec::diagnostics::Diagnostic,
) -> Option<Diagnostic> {
    let severity = match diagnostic.level() {
        slicec::diagnostics::DiagnosticLevel::Error => Some(DiagnosticSeverity::ERROR),
        slicec::diagnostics::DiagnosticLevel::Warning => Some(DiagnosticSeverity::WARNING),
        // Ignore the allowed level
        _ => None,
    };

    // Map the spans to ranges, if span is none, return none
    let range = match diagnostic.span() {
        Some(span) => {
            let start = Position::new((span.start.row - 1) as u32, (span.start.col - 1) as u32);
            let end = Position::new((span.end.row - 1) as u32, (span.end.col - 1) as u32);
            Range::new(start, end)
        }
        None => return None,
    };

    let message = diagnostic.message();

    Some(Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diagnostic.code().to_owned())),
        code_description: Some(CodeDescription { href:
            // Create a URL object to https://docs.icerpc.dev
            Url::parse("https://docs.icerpc.dev").unwrap(),
         }),
        source: Some("slicec".to_owned()),
        message,
        related_information: None,
        tags: None,
        data: None,
    })
}

fn built_in_keywords() -> Vec<CompletionItem> {
    return vec![
        CompletionItem {
            label: "module".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "struct".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "exception".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "class".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "interface".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "enum".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "custom".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "typealias".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "Sequence".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "Dictionary".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "bool".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "int8".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "uint8".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "int16".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "uint16".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "int32".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "uint32".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "varint32".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "varuint32".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "int64".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "uint64".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "varint62".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "varuint62".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "float32".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "float64".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "string".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "AnyClass".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "compact".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "idempotent".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "mode".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "stream".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "tag".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "throws".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
        CompletionItem {
            label: "unchecked".to_owned(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Foo".to_owned()),
            ..Default::default()
        },
    ];
}

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
