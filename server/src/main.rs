// Copyright (c) ZeroC, Inc.

use config::SliceConfig;
use diagnostic_ext::try_into_lsp_diagnostic;
use hover::get_hover_info;
use jump_definition::get_definition_span;
use shared_state::SharedState;
use slicec::{compilation_state::CompilationState, slice_options::SliceOptions};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use tower_lsp::{lsp_types::*, Client, LanguageServer, LspService, Server};
use utils::convert_slice_url_to_uri;

mod config;
mod diagnostic_ext;
mod hover;
mod jump_definition;
mod shared_state;
mod utils;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        slice_config: Arc::new(Mutex::new(SliceConfig::default())),
        client,
        shared_state: Arc::new(Mutex::new(SharedState::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

struct Backend {
    client: Client,
    slice_config: Arc<Mutex<SliceConfig>>,
    shared_state: Arc<Mutex<SharedState>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        // Use the root_uri if it exists temporarily as we cannot access configuration until
        // after initialization. Additionally, LSP may provide the windows path with escaping or a lowercase
        // drive letter. To fix this, we convert the path to a URL and then back to a path.
        if let Some(root_uri) = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .and_then(|path| Url::from_file_path(path).ok())
        {
            let mut slice_config = self.slice_config.lock().await;
            slice_config.set_root_uri(root_uri.clone());
        }

        // Update the slice configuration with the built-in reference path
        if let Some(options) = params.initialization_options {
            if let Some(built_in_slice_path) =
                options.get("builtInSlicePath").and_then(|v| v.as_str())
            {
                let mut slice_config = self.slice_config.lock().await;
                slice_config.set_built_in_reference(built_in_slice_path);
            }
        }

        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
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
        // Update the slice configuration with new references directories since this is the first time we can access
        // the slice extension configuration
        {
            // TODO: Log error
            let _ = self
                .slice_config
                .lock()
                .await
                .try_update_from_client(&self.client)
                .await;
        }

        let mut shared_state_lock = self.shared_state.lock().await;

        // Compile the Slice files and publish diagnostics
        let (updated_state, options) = self.compile_slice_files().await;

        shared_state_lock.compilation_state = updated_state;
        shared_state_lock.compilation_options = options;

        self.client
            .log_message(MessageType::INFO, "Slice Language Server initialized")
            .await;

        self.publish_diagnostics_for_all_files(&mut shared_state_lock)
            .await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Slice Language Server config changed")
            .await;

        // Update the slice configuration
        {
            let mut slice_config = self.slice_config.lock().await;
            slice_config.update_from_params(&params);
        }

        // Store the current files in the compilation state before re-compiling
        let current_files = &self
            .shared_state
            .lock()
            .await
            .compilation_state
            .files
            .keys()
            .cloned()
            .collect::<HashSet<_>>();

        // Re-compile the Slice files considering the updated references
        let (updated_state, options) = self.compile_slice_files().await;

        // Clear the diagnostics from files that are no longer in the compilation state
        let new_files = &updated_state.files.keys().cloned().collect::<HashSet<_>>();
        for url in current_files.difference(new_files) {
            let Some(uri) = convert_slice_url_to_uri(url) else {
                continue;
            };
            self.client.publish_diagnostics(uri, vec![], None).await;
        }

        let mut shared_state_lock = self.shared_state.lock().await;

        shared_state_lock.compilation_state = updated_state;
        shared_state_lock.compilation_options = options;

        self.publish_diagnostics_for_all_files(&mut shared_state_lock)
            .await;
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

        let location = match get_definition_span(compilation_state, param_uri, position) {
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

    async fn did_open(&self, _: DidOpenTextDocumentParams) {
        self.handle_file_change().await;
    }

    async fn did_save(&self, _: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved")
            .await;
        self.handle_file_change().await;
    }
}

impl Backend {
    async fn handle_file_change(&self) {
        let (updated_state, options) = self.compile_slice_files().await;

        let mut shared_state_lock = self.shared_state.lock().await;

        shared_state_lock.compilation_state = updated_state;
        shared_state_lock.compilation_options = options;

        self.publish_diagnostics_for_all_files(&mut shared_state_lock)
            .await;
    }

    async fn compile_slice_files(&self) -> (CompilationState, SliceOptions) {
        self.client
            .log_message(MessageType::INFO, "compiling slice")
            .await;

        let references = self.slice_config.lock().await.get_resolved_reference_paths().to_vec();

        // If debug is enabled, log the resolved references
        #[cfg(debug_assertions)]
        self.client
            .log_message(MessageType::LOG, format!("references: {:?}", references))
            .await;

        // Compile the Slice files
        let options = SliceOptions {
            references,
            ..Default::default()
        };
        (
            slicec::compile_from_options(&options, |_| {}, |_| {}),
            options,
        )
    }

    async fn publish_diagnostics_for_all_files(&self, shared_state: &mut SharedState) {
        let compilation_options = &shared_state.compilation_options;
        let compilation_state = &mut shared_state.compilation_state;

        let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
            &compilation_state.ast,
            &compilation_state.files,
            compilation_options,
        );

        // Group the diagnostics by file since diagnostics are published per file and diagnostic.span contains the file URL
        let mut map = compilation_state
            .files
            .keys()
            .filter_map(|uri| Some((convert_slice_url_to_uri(uri)?, vec![])))
            .collect::<HashMap<Url, Vec<Diagnostic>>>();

        // Add the diagnostics to the map
        for diagnostic in diagnostics {
            let Some(span) = diagnostic.span() else {
                continue;
            };
            let Some(uri) = convert_slice_url_to_uri(&span.file) else {
                continue;
            };
            let Some(lsp_diagnostic) = try_into_lsp_diagnostic(&diagnostic) else {
                continue;
            };
            map.get_mut(&uri)
                .expect("file not in map")
                .push(lsp_diagnostic)
        }

        for (uri, lsp_diagnostics) in map {
            self.client
                .publish_diagnostics(uri, lsp_diagnostics, None)
                .await;
        }
        self.client
            .log_message(MessageType::LOG, "Updated diagnostics for all files")
            .await;
    }
}
