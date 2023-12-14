// Copyright (c) ZeroC, Inc.

use config::SliceConfig;
use diagnostic_ext::try_into_lsp_diagnostic;
use hover::get_hover_info;
use jump_definition::get_definition_span;
use serde_json::Value;
use slicec::compilation_state::CompilationState;
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
mod utils;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        configuration_sets: Arc::new(Mutex::new(HashMap::new())),
        root_uri: Arc::new(Mutex::new(None)),
        built_in_slice_path: Arc::new(Mutex::new(String::new())),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

struct Backend {
    client: Client,
    configuration_sets: Arc<Mutex<HashMap<SliceConfig, CompilationState>>>,
    root_uri: Arc<Mutex<Option<Url>>>,
    built_in_slice_path: Arc<Mutex<String>>,
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
            // Store the root_uri in the backend
            let mut root_uri_lock = self.root_uri.lock().await;
            *root_uri_lock = Some(root_uri.clone());

            // This is the path to the built-in Slice files that are included with the extension. It should always
            // be present.
            let built_in_slice_path = params
                .initialization_options
                .unwrap()
                .get("builtInSlicePath")
                .and_then(|v| v.as_str())
                .unwrap()
                .to_string();

            // Store the built_in_slice_path in the backend
            let mut built_in_slice_path_lock = self.built_in_slice_path.lock().await;
            *built_in_slice_path_lock = built_in_slice_path.clone();

            // Insert the default configuration set
            let mut configuration_sets = self.configuration_sets.lock().await;

            // Default configuration set
            let default =
                new_default_configuration_set(root_uri.clone(), built_in_slice_path.clone());
            configuration_sets.insert(default.0, default.1);
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
        let found_configurations = self.fetch_configurations().await.unwrap_or_default();
        self.update_configuration_sets(found_configurations).await;
        self.publish_diagnostics().await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Slice Language Server config changed")
            .await;
        self.clear_diagnostics().await;
        let found_configurations = self.parse_configuration(params).await.unwrap_or_default();
        self.update_configuration_sets(found_configurations).await;
        self.clear_diagnostics().await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = Url::from_file_path(
            params
                .text_document_position_params
                .text_document
                .uri
                .to_file_path()
                .unwrap(),
        )
        .unwrap();

        let file_name = params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let position = params.text_document_position_params.position;

        // Find the configuration set that contains the file
        let configuration_sets = self.configuration_sets.lock().await;

        let configuration_set = configuration_sets.iter().find(|config| {
            // Find the configuration set that matches the current configuration
            let files = config.1.files.keys().cloned().collect::<Vec<_>>();
            files.contains(&file_name.to_owned())
        });
        let compilation_state = configuration_set.map(|config| config.1).unwrap();

        let location = match get_definition_span(compilation_state, uri, position) {
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

        let file_name = params
            .text_document_position_params
            .text_document
            .uri
            .to_file_path()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();

        let position = params.text_document_position_params.position;

        // Find the configuration set that contains the file
        let configuration_sets = self.configuration_sets.lock().await;

        let configuration_set = configuration_sets.iter().find(|config| {
            // Find the configuration set that matches the current configuration
            let files = config.1.files.keys().cloned().collect::<Vec<_>>();
            files.contains(&file_name.to_owned())
        });

        // Log the configuration_set
        if let Some(compilation_state) = configuration_set.map(|config| config.1) {
            Ok(
                get_hover_info(compilation_state, uri, position).map(|info| Hover {
                    contents: HoverContents::Scalar(MarkedString::String(info)),
                    range: None,
                }),
            )
        } else {
            // Log the configuration_set
            Ok(None)
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let file_name = params
            .text_document
            .uri
            .to_file_path()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        self.handle_file_change(&file_name).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let file_name = params
            .text_document
            .uri
            .to_file_path()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        self.handle_file_change(&file_name).await;
    }
}

impl Backend {
    async fn handle_file_change(&self, file_name: &str) {
        self.client
            .log_message(MessageType::INFO, format!("File {} changed", file_name))
            .await;

        let mut configuration_sets = self.configuration_sets.lock().await;

        // Update the compilation state for the any impacted configuration set
        configuration_sets
            .iter_mut()
            .filter(|config| {
                // Find the configuration set that matches the current configuration
                let files = config.1.files.keys().cloned().collect::<Vec<_>>();
                files.contains(&file_name.to_owned())
            })
            .for_each(|configuration_set| {
                // Update the value of the compilation state in the configuration set in the HashMap
                *(configuration_set).1 = slicec::compile_from_options(
                    configuration_set.0.as_slice_options(),
                    |_| {},
                    |_| {},
                );
            });

        // TEMP
        let mut publish_map = HashMap::new();

        // The diagnostics for each configuration set
        let diagnostic_sets = configuration_sets
            .iter_mut()
            .filter(|config| {
                // Find the configuration set that matches the current configuration
                let files = config.1.files.keys().cloned().collect::<Vec<_>>();
                files.contains(&file_name.to_owned())
            })
            .map(|configuration_set| {
                let compilation_state = configuration_set.1;

                // Find the diagnostics for the config set
                let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
                    &compilation_state.ast,
                    &compilation_state.files,
                    configuration_set.0.as_slice_options(),
                );

                // Store all files in the configuration set
                compilation_state
                    .files
                    .keys()
                    .filter_map(|uri| convert_slice_url_to_uri(uri))
                    .for_each(|uri| {
                        publish_map.insert(uri, vec![]);
                    });
                diagnostics
            })
            .collect::<Vec<_>>();

        // Flatten and deduplicate the diagnostics based on their span
        let mut diagnostics = diagnostic_sets.iter().flatten().collect::<Vec<_>>();
        diagnostics.dedup_by(|d1, d2| d1.span() == d2.span());

        // Group the diagnostics by file since diagnostics are published per file and diagnostic.span contains the file URL
        for diagnostic in diagnostics {
            let Some(span) = diagnostic.span() else {
                continue;
            };
            let Some(uri) = convert_slice_url_to_uri(&span.file) else {
                continue;
            };
            let Some(lsp_diagnostic) = try_into_lsp_diagnostic(diagnostic) else {
                continue;
            };
            publish_map
                .get_mut(&uri)
                .expect("file not in map")
                .push(lsp_diagnostic)
        }

        self.client
            .log_message(MessageType::INFO, format!("publish_map {:?}", publish_map))
            .await;

        // Publish the diagnostics for each file
        for (uri, lsp_diagnostics) in publish_map {
            self.client
                .publish_diagnostics(uri, lsp_diagnostics, None)
                .await;
        }

        self.client
            .log_message(MessageType::INFO, "Updated diagnostics for all files")
            .await;
    }

    async fn publish_diagnostics_for_all_files(
        &self,
        configuration_set: (&SliceConfig, &mut CompilationState),
    ) {
        self.client
            .log_message(MessageType::INFO, "Publishing diagnostics...")
            .await;
        let compilation_state = configuration_set.1;

        let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
            &compilation_state.ast,
            &compilation_state.files,
            configuration_set.0.as_slice_options(),
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
            .log_message(MessageType::INFO, "Updated diagnostics for all files")
            .await;
    }

    async fn fetch_configurations(&self) -> Option<Vec<(SliceConfig, CompilationState)>> {
        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.configurations".to_string()),
        }];

        let response = self.client.configuration(params).await.ok()?;
        let root_uri = self.root_uri.lock().await;
        Some(parse_slice_configuration_sets(
            response.first()?.as_array()?.to_vec(),
            &((*root_uri).clone().unwrap()),
        ))
    }

    // New function to update configuration sets
    async fn update_configuration_sets(
        &self,
        configurations: Vec<(SliceConfig, CompilationState)>,
    ) {
        let mut configuration_sets = self.configuration_sets.lock().await;
        *configuration_sets = configurations.into_iter().collect();
    }

    // New function to update diagnostics for all files
    async fn publish_diagnostics(&self) {
        let mut configuration_sets = self.configuration_sets.lock().await;
        for configuration_set in configuration_sets.iter_mut() {
            self.publish_diagnostics_for_all_files(configuration_set)
                .await;
        }
    }

    // Clear the diagnostics for all tracked files
    async fn clear_diagnostics(&self) {
        let configuration_sets = self.configuration_sets.lock().await;
        let mut all_tracked_files = HashSet::new();
        for configuration_set in configuration_sets.iter() {
            configuration_set
                .1
                .files
                .keys()
                .cloned()
                .filter_map(|uri| convert_slice_url_to_uri(&uri))
                .for_each(|uri| {
                    all_tracked_files.insert(uri);
                });
        }

        for uri in all_tracked_files {
            self.client.publish_diagnostics(uri, vec![], None).await;
        }
    }

    async fn parse_configuration(
        &self,
        params: DidChangeConfigurationParams,
    ) -> Option<Vec<(SliceConfig, CompilationState)>> {
        let root_uri = self.root_uri.lock().await;
        params
            .settings
            .get("slice.configurations")
            .and_then(|v| v.as_array())
            .map(|config_array| {
                parse_slice_configuration_sets(config_array.to_vec(), &(*root_uri).clone().unwrap())
            })
    }
}

fn new_default_configuration_set(
    root_uri: Url,
    built_in_path: String,
) -> (SliceConfig, CompilationState) {
    let mut configuration = SliceConfig::default();
    configuration.set_root_uri(root_uri);
    configuration.set_built_in_reference(built_in_path.to_owned());
    let compilation_state =
        slicec::compile_from_options(configuration.as_slice_options(), |_| {}, |_| {});
    (configuration, compilation_state)
}

fn parse_slice_configuration_sets(
    config_array: Vec<Value>,
    root_uri: &Url,
) -> Vec<(SliceConfig, CompilationState)> {
    config_array
        .iter()
        .filter_map(|config| config.as_object())
        .map(|config_obj| {
            let directories = config_obj
                .get("referenceDirectories")
                .and_then(|v| v.as_array())
                .map(|dirs_array| {
                    dirs_array
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let include_built_in = config_obj
                .get("includeBuiltInTypes")
                .and_then(|v| v.as_bool())
                .unwrap_or_default();

            (directories, include_built_in)
        })
        .map(|config| {
            let mut slice_config = SliceConfig::default();

            slice_config.set_root_uri(root_uri.clone());
            slice_config.update_from_references(config.0);
            slice_config.update_include_built_in_reference(config.1);

            let options = slice_config.as_slice_options();

            let compilation_state = slicec::compile_from_options(options, |_| {}, |_| {});

            (slice_config, compilation_state)
        })
        .collect::<Vec<_>>()
}
