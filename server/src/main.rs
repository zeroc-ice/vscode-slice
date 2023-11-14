// Copyright (c) ZeroC, Inc.

use diagnostic_ext::try_into_lsp_diagnostic;
use hover::get_hover_info;
use jump_definition::get_definition_span;
use shared_state::SharedState;
use slicec::{compilation_state::CompilationState, slice_options::SliceOptions};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;
use tower_lsp::{lsp_types::*, Client, LanguageServer, LspService, Server};
use utils::convert_slice_url_to_uri;

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
        sources_uri: Arc::new(Mutex::new(None)),
        reference_uris: Arc::new(Mutex::new(None)),
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
    reference_uris: Arc<Mutex<Option<Vec<Url>>>>,
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
        // Update the sources and references directories
        {
            let workspace_uri = self.workspace_uri.lock().await;

            // Fetch and set the sources directory
            let sources_directory = self.get_sources_directory(&workspace_uri, None).await.ok();
            let references_directories = self
                .get_reference_directories(&workspace_uri, None)
                .await
                .ok();
            let mut sources_uri = self.sources_uri.lock().await;
            let mut references_uris = self.reference_uris.lock().await;
            *sources_uri = sources_directory;
            *references_uris = references_directories;
        }

        let mut shared_state_lock = self.shared_state.lock().await;

        // Compile the Slice files and publish diagnostics
        let (updated_state, options) = self.compile_slice_files().await;

        shared_state_lock.compilation_state = updated_state;
        shared_state_lock.compilation_options = options;

        self.publish_diagnostics_for_all_files(&mut shared_state_lock)
            .await;

        self.client
            .log_message(MessageType::INFO, "Slice language server initialized")
            .await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Slice language server config changed")
            .await;

        // Update the sources directory if it has changed
        let (sources_directory, reference_directories) = {
            let workspace_uri = self.workspace_uri.lock().await;
            let s = self
                .get_sources_directory(&workspace_uri, Some(&params))
                .await
                .ok();

            let r = self
                .get_reference_directories(&workspace_uri, Some(&params))
                .await
                .ok();
            (s, r)
        };

        // Check if either sources or reference directories have changed
        if sources_directory.is_some() || reference_directories.is_some() {
            {
                if let Some(sources_dir) = sources_directory {
                    *self.sources_uri.lock().await = Some(sources_dir);
                }

                if let Some(ref_dirs) = reference_directories {
                    *self.reference_uris.lock().await = Some(ref_dirs);
                }
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

            // Re-compile the Slice files considering both sources and references
            let (updated_state, options) = self.compile_slice_files().await;

            // Clear the diagnostics from files that are no longer in the compilation state
            let new_files = &updated_state.files.keys().cloned().collect::<HashSet<_>>();

            let clear_diagnostic_tasks = current_files
                .difference(new_files)
                .filter_map(|url| convert_slice_url_to_uri(url))
                .map(|uri| self.client.publish_diagnostics(uri, vec![], None));

            futures::future::join_all(clear_diagnostic_tasks).await;

            let mut shared_state_lock = self.shared_state.lock().await;

            shared_state_lock.compilation_state = updated_state;
            shared_state_lock.compilation_options = options;

            self.publish_diagnostics_for_all_files(&mut shared_state_lock)
                .await;
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.handle_file_change(params.text_document.uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "file saved")
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
                        (span_path == uri_path).then_some(d)
                    })
                    // Convert to LSP diagnostic
                    .and_then(try_into_lsp_diagnostic)
            })
            .collect::<Vec<_>>()
    }

    async fn compile_slice_files(&self) -> (CompilationState, SliceOptions) {
        self.client
            .log_message(MessageType::INFO, "compiling slice")
            .await;
        let workspace_uri = self.workspace_uri.lock().await;
        let workspace_path = workspace_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok());
        let sources_uri = self.sources_uri.lock().await;
        let source_path = sources_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok())
            .map(|path| path.display().to_string());

        let reference_uris = self.reference_uris.lock().await;
        let reference_paths = reference_uris
            .as_ref()
            .map(|uris| {
                uris.iter()
                    .map(|uri| {
                        if let Ok(path) = uri.to_file_path() {
                            // If the path is already absolute, use it as is
                            if path.is_absolute() {
                                path.display().to_string()
                            } else if let Some(workspace_path) = &workspace_path {
                                // Otherwise, resolve it relative to the workspace root
                                workspace_path.join(path).display().to_string()
                            } else {
                                String::new() // Empty string for paths that cannot be resolved
                            }
                        } else {
                            String::new() // Empty string for invalid URIs
                        }
                    })
                    .filter(|s| !s.is_empty()) // Filter out empty strings
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // This case should never happen during normal operation. In case it does, we log an error before unwrapping.
        if source_path.is_none() {
            self.client.log_message(
                MessageType::ERROR,
                format!("Could not get sources directory path during slice compilation. Sources URI {:?}", sources_uri)
            )
            .await;
        }

        // Combine the sources and references into a single vec of strings
        let mut combined_paths = Vec::new();
        if let Some(src_path) = source_path {
            combined_paths.push(src_path);
        }
        combined_paths.extend(reference_paths);

        // Compile the Slice files
        let options = SliceOptions {
            references: combined_paths,
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
        config_params: Option<&DidChangeConfigurationParams>,
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
                .first()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or(None)?
        };

        if let Some(workspace_dir) = workspace_root {
            let workspace_path = workspace_dir.to_file_path().map_err(|_| {
                Some(tower_lsp::jsonrpc::Error::invalid_params(
                    "Failed to convert workspace URL to file path.",
                ))
            })?;

            let sources_path = workspace_path.join(sources_directory);

            Url::from_file_path(sources_path).map_err(|_| {
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

    async fn get_reference_directories(
        &self,
        workspace_root: &Option<Url>,
        config_params: Option<&DidChangeConfigurationParams>,
    ) -> Result<Vec<Url>, Option<tower_lsp::jsonrpc::Error>> {
        let reference_directories: Vec<String> = if let Some(params) = config_params {
            params
                .settings
                .get("slice")
                .and_then(|v| v.get("referenceDirectory"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .ok_or(None)?
        } else {
            let params = vec![ConfigurationItem {
                scope_uri: None,
                section: Some("slice.referenceDirectory".to_string()),
            }];

            let result = self.client.configuration(params).await?;
            result
                .first()
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .ok_or(None)?
        };

        if let Some(workspace_dir) = workspace_root {
            let workspace_path = workspace_dir.to_file_path().map_err(|_| {
                Some(tower_lsp::jsonrpc::Error::invalid_params(
                    "Failed to convert workspace URL to file path.",
                ))
            })?;

            let urls = reference_directories
                .iter()
                .map(|dir| {
                    let path = workspace_path.join(dir);
                    Url::from_file_path(path).map_err(|_| {
                        Some(tower_lsp::jsonrpc::Error::invalid_params(
                            "Failed to convert reference path to URL.",
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(urls)
        } else {
            Err(Some(tower_lsp::jsonrpc::Error::invalid_params(
                "Workspace directory is not set.",
            )))
        }
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
        let mut map: HashMap<Url, Vec<Diagnostic>> = HashMap::new();

        // Add an empty vector for each file in the compilation state so they all get updated
        compilation_state
            .files
            .keys()
            .map(|k| k.as_str())
            .filter_map(convert_slice_url_to_uri)
            .for_each(|url| {
                map.insert(url, vec![]);
            });

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
            map.entry(uri).or_default().push(lsp_diagnostic)
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
