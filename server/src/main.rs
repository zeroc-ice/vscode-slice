// Copyright (c) ZeroC, Inc.

use crate::utils::FindFile;
use config::SliceConfig;
use diagnostic_ext::{clear_diagnostics, process_diagnostics, publish_diagnostics};
use hover::try_into_hover_result;
use jump_definition::get_definition_span;
use slicec::compilation_state::CompilationState;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tower_lsp::{jsonrpc::Error, lsp_types::*, Client, LanguageServer, LspService, Server};
use utils::{
    convert_slice_url_to_uri, new_configuration_set, parse_slice_configuration_sets,
    url_to_file_path,
};

mod config;
mod diagnostic_ext;
mod hover;
mod jump_definition;
mod utils;

type ConfigurationSet = (SliceConfig, CompilationState);

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
    // This HashMap contains all of the configuration sets for the language server. The key is the SliceConfig and the
    // value is the CompilationState. The SliceConfig is used to determine which configuration set to use when
    // publishing diagnostics. The CompilationState is used to retrieve the diagnostics for a given file.
    configuration_sets: Arc<Mutex<HashMap<SliceConfig, CompilationState>>>,
    // This is the root URI of the workspace. It is used to resolve relative paths in the configuration.
    root_uri: Arc<Mutex<Option<Url>>>,
    // This is the path to the built-in Slice files that are included with the extension.
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
            .map(|uri| async {
                *self.root_uri.lock().await = Some(uri.clone());
                uri
            })
        {
            // Wait for the root_uri to be set
            let root_uri = root_uri.await;

            // This is the path to the built-in Slice files that are included with the extension. It should always
            // be present.
            let built_in_slice_path = params
                .initialization_options
                .and_then(|opts| opts.get("builtInSlicePath").cloned())
                .and_then(|v| v.as_str().map(str::to_owned))
                .expect("builtInSlicePath not found in initialization options");

            *self.built_in_slice_path.lock().await = built_in_slice_path.clone();

            // Insert the default configuration set
            let mut configuration_sets = self.configuration_sets.lock().await;

            // Default configuration set
            let default = new_configuration_set(root_uri.clone(), built_in_slice_path.clone());
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
        let found_configurations = self.fetch_settings().await;

        // Update the configuration sets with the new configurations if any were found. Otherwise, leave the default
        // configuration set in place.
        if !found_configurations.is_empty() {
            // When the configuration changes, any of the files in the workspace could be impacted. Therefore, we need to
            // clear the diagnostics for all files and then re-publish them.
            clear_diagnostics(&self.client, &self.configuration_sets).await;

            // Update the configuration sets
            self.update_configuration_sets(found_configurations).await;
            publish_diagnostics(&self.client, &self.configuration_sets).await;
        }
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Slice Language Server settings changed")
            .await;

        // When the configuration changes, any of the files in the workspace could be impacted. Therefore, we need to
        // clear the diagnostics for all files and then re-publish them.
        clear_diagnostics(&self.client, &self.configuration_sets).await;

        // Parse the new configurations and update the configuration sets
        let found_configurations = self.parse_updated_settings(params).await;
        self.update_configuration_sets(found_configurations).await;

        // Publish the diagnostics for all files
        publish_diagnostics(&self.client, &self.configuration_sets).await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Convert the URI to a file path and back to a URL to ensure that the URI is formatted correctly for Windows.
        let file_name = url_to_file_path(&uri).ok_or_else(Error::internal_error)?;
        let url = uri
            .to_file_path()
            .and_then(Url::from_file_path)
            .map_err(|_| Error::internal_error())?;

        // Find the configuration set that contains the file
        let configuration_sets = self.configuration_sets.lock().await;
        let compilation_state = configuration_sets
            .iter()
            .find_file(&file_name)
            .map(|config| config.1);

        // Get the definition span and convert it to a GotoDefinitionResponse
        compilation_state
            .and_then(|state| get_definition_span(state, url, position))
            .and_then(|location| {
                let start = Position {
                    line: (location.start.row - 1) as u32,
                    character: (location.start.col - 1) as u32,
                };
                let end = Position {
                    line: (location.end.row - 1) as u32,
                    character: (location.end.col - 1) as u32,
                };
                Url::from_file_path(location.file).ok().map(|uri| {
                    GotoDefinitionResponse::Scalar(Location {
                        uri,
                        range: Range::new(start, end),
                    })
                })
            })
            .map_or(Ok(None), |resp| Ok(Some(resp)))
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Convert the URI to a file path and back to a URL to ensure that the URI is formatted correctly for Windows.
        let file_name = url_to_file_path(&uri).ok_or_else(Error::internal_error)?;
        let url = uri
            .to_file_path()
            .and_then(Url::from_file_path)
            .map_err(|_| Error::internal_error())?;

        // Find the configuration set that contains the file and get the hover info
        let configuration_sets = self.configuration_sets.lock().await;
        Ok(configuration_sets
            .iter()
            .find_file(&file_name)
            .map(|config| config.1)
            .and_then(|compilation_state| try_into_hover_result(compilation_state, url, position)))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Some(file_name) = url_to_file_path(&params.text_document.uri) {
            self.handle_file_change(&file_name).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(file_name) = url_to_file_path(&params.text_document.uri) {
            self.handle_file_change(&file_name).await;
        }
    }
}

impl Backend {
    async fn handle_file_change(&self, file_name: &str) {
        self.client
            .log_message(MessageType::INFO, format!("File {} changed", file_name))
            .await;

        let mut publish_map = HashMap::new();
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

        // Collect the diagnostics for each configuration set
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
        // Process diagnostics and update publish_map
        process_diagnostics(diagnostics, &mut publish_map);

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

    // Fetch the configurations from the client and parse them into configuration sets.
    async fn fetch_settings(&self) -> Vec<ConfigurationSet> {
        let root_uri = self.root_uri.lock().await;
        let built_in_slice_path = &self.built_in_slice_path.lock().await;
        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.configurations".to_string()),
        }];

        // Fetch the configurations from the client, parse them, and return the configuration sets. If no configurations
        // are found, return an empty vector.
        self.client
            .configuration(params)
            .await
            .ok()
            .and_then(|response| {
                root_uri.as_ref().map(|uri| {
                    parse_slice_configuration_sets(
                        &response
                            .iter()
                            .filter_map(|config| config.as_array())
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>(),
                        uri,
                        built_in_slice_path,
                    )
                })
            })
            .unwrap_or_default()
    }

    // Parse the updated settings and return the configuration sets. If no configurations are found, return an empty
    // vector.
    async fn parse_updated_settings(
        &self,
        params: DidChangeConfigurationParams,
    ) -> Vec<ConfigurationSet> {
        let root_uri = self.root_uri.lock().await;
        let built_in_slice_path = &self.built_in_slice_path.lock().await;

        params
            .settings
            .get("slice")
            .and_then(|v| v.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|config_array| {
                parse_slice_configuration_sets(
                    config_array,
                    &(*root_uri).clone().unwrap(),
                    built_in_slice_path,
                )
            })
            .unwrap_or_default()
    }

    // Update the configuration sets with the new configurations. If there are no configuration sets after updating,
    // insert the default configuration set.
    async fn update_configuration_sets(&self, configurations: Vec<ConfigurationSet>) {
        let mut configuration_sets = self.configuration_sets.lock().await;
        *configuration_sets = configurations.into_iter().collect();

        // Insert the default configuration set if needed
        if configuration_sets.is_empty() {
            let root_uri = self.root_uri.lock().await;
            let built_in_slice_path = self.built_in_slice_path.lock().await;
            let default =
                new_configuration_set(root_uri.clone().unwrap(), built_in_slice_path.clone());
            configuration_sets.insert(default.0, default.1);
        }
    }
}
