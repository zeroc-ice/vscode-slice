// Copyright (c) ZeroC, Inc.

use diagnostic_ext::try_into_lsp_diagnostic;
use hover::get_hover_info;
use jump_definition::get_definition_span;
use shared_state::SharedState;
use slicec::{compilation_state::CompilationState, slice_options::SliceOptions};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tower_lsp::{lsp_types::*, Client, LanguageServer, LspService, Server};

mod diagnostic_ext;
mod hover;
mod jump_definition;
mod shared_state;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        sources_uri: Arc::new(Mutex::new(None)),
        client,
        workspace_uri: Arc::new(Mutex::new(None)),
        shared_state: Arc::new(Mutex::new(SharedState::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

struct Backend {
    client: Client,
    // The workspace URI is the URI of the VSCode workspace.
    workspace_uri: Arc<Mutex<Option<Url>>>,
    // The sources URI is the URI of the directory containing the Slice files.
    sources_uri: Arc<Mutex<Option<Url>>>,
    shared_state: Arc<Mutex<SharedState>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        // Use the workspace root if it exists temporarily as we cannot access configuration until
        // after initialization. Additionally, LSP may provide the windows path with escaping or a lowercase
        // drive letter. To fix this, we convert the path to a URL and then back to a path.
        let fixed_uri = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .and_then(|path| Url::from_file_path(path).ok());
        *self.workspace_uri.lock().await = fixed_uri.clone();
        *self.sources_uri.lock().await = fixed_uri;

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
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let workspace_uri = self.workspace_uri.lock().await;

        // Fetch and set the search directory
        let sources_directory = self.get_sources_directory(&workspace_uri, None).await.ok();
        let mut sources_uri = self.sources_uri.lock().await;
        *sources_uri = sources_directory;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Slice language server config changed!")
            .await;
        let workspace_uri = self.workspace_uri.lock().await;

        // Update the search directory if it has changed
        let sources_directory = self
            .get_sources_directory(&workspace_uri, Some(params))
            .await
            .ok();

        if sources_directory.is_some() {
            *self.sources_uri.lock().await = sources_directory;

            // Re-compile the Slice files
            let (updated_state, options) = self.compile_slice_files().await;
            let mut shared_state_lock = self.shared_state.lock().await;

            shared_state_lock.compilation_state = updated_state;
            shared_state_lock.compilation_options = options;
        } else {
            self.client
                .log_message(
                    MessageType::ERROR,
                    "Failed to update sources directory. Please check your Slice language server configuration.",
                )
                .await;
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let param_uri = Url::from_file_path(
            params
                .text_document_position_params
                .text_document
                .uri
                .to_file_path()
                .unwrap(),
        )
        .unwrap();

        let position = params.text_document_position_params.position;
        let compilation_state = &self.shared_state.lock().await.compilation_state;

        let location = match get_definition_span(&compilation_state, param_uri, position) {
            Some(location) => location,
            None => return Ok(None),
        };
        let start = Position {
            line: (location.start.row - 1) as u32,
            character: (location.start.col - 1) as u32,
        };

        let end = Position {
            line: (location.end.row - 1) as u32,
            character: (location.end.col - 1) as u32,
        };

        let Ok(uri) = Url::from_file_path(location.file) else {
            return Ok(None);
        };

        Ok(Some(GotoDefinitionResponse::Scalar(Location {
            uri,
            range: Range::new(start, end),
        })))
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = Url::from_file_path(
            params
                .text_document_position_params
                .text_document
                .uri
                .to_file_path()
                .unwrap(),
        )
        .unwrap();
        let position = params.text_document_position_params.position;
        let compilation_state = &self.shared_state.lock().await.compilation_state;
        Ok(
            get_hover_info(compilation_state, uri, position).map(|info| Hover {
                contents: HoverContents::Scalar(MarkedString::String(info)),
                range: None,
            }),
        )
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

        let mut shared_state_lock = self.shared_state.lock().await;

        shared_state_lock.compilation_state = updated_state;
        shared_state_lock.compilation_options = options;

        let diagnostics = self
            .get_diagnostics_for_uri(&uri, &mut shared_state_lock)
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
        shared_state: &mut SharedState,
    ) -> Vec<Diagnostic> {
        let compilation_state = &mut shared_state.compilation_state;

        let options = &shared_state.compilation_options;
        let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
            &compilation_state.ast,
            &compilation_state.files,
            options,
        );

        self.client
            .log_message(
                MessageType::INFO,
                format!("got {} diagnostics", diagnostics.len()),
            )
            .await;

        diagnostics
            .iter()
            .filter_map(|d| {
                d.span()
                    .and_then(|s| {
                        // Convert the span file path to a PathBuf
                        let span_path = PathBuf::from(&s.file);

                        // Convert the URI to a PathBuf, handling URL decoding
                        let uri_path = uri.to_file_path().ok().map(|p| p.to_path_buf())?;

                        // Check if the paths match
                        (span_path == uri_path).then(|| d)
                    })
                    // Convert to LSP diagnostic
                    .and_then(|d| try_into_lsp_diagnostic(d))
            })
            .collect::<Vec<_>>()
    }

    async fn compile_slice_files(&self) -> (CompilationState, SliceOptions) {
        self.client
            .log_message(MessageType::INFO, "compiling slice")
            .await;

        let sources_uri = self.sources_uri.lock().await;

        let directory_path = sources_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok())
            .map(|path| path.display().to_string());

        // This case should never happen during normal operation. In case it does, we log an error before unwrapping.
        if directory_path.is_none() {
            self.client.log_message(
                MessageType::ERROR,
                format!("Could not get sources directory path during slice compilation. Sources URI {:?}", sources_uri)
            )
            .await;
        }

        let options = SliceOptions {
            references: vec![directory_path.unwrap()],
            ..Default::default()
        };
        (
            slicec::compile_from_options(&options, |_| {}, |_| {}),
            options,
        )
    }

    async fn get_sources_directory(
        &self,
        workspace_root: &Option<Url>,
        config_params: Option<DidChangeConfigurationParams>,
    ) -> Result<Url, Option<tower_lsp::jsonrpc::Error>> {
        let sources_directory = if let Some(params) = config_params {
            params
                .settings
                .get("slice")
                .and_then(|v| v.get("sourceDirectory"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or(None)?
        } else {
            let params = vec![ConfigurationItem {
                scope_uri: None,
                section: Some("slice.sourceDirectory".to_string()),
            }];

            let result = self.client.configuration(params).await?;
            result
                .get(0)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or(None)?
        };

        if let Some(workspace_dir) = workspace_root {
            let workspace_path = workspace_dir.to_file_path().map_err(|_| {
                Some(tower_lsp::jsonrpc::Error::invalid_params(
                    "Failed to convert workspace URL to file path.",
                ))
            })?;

            let search_path = workspace_path.join(&sources_directory);

            Url::from_file_path(search_path).map_err(|_| {
                Some(tower_lsp::jsonrpc::Error::invalid_params(
                    "Failed to convert search path to URL.",
                ))
            })
        } else {
            Err(Some(tower_lsp::jsonrpc::Error::invalid_params(
                "Workspace directory is not set.",
            )))
        }
    }
}
