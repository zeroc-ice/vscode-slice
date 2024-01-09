// Copyright (c) ZeroC, Inc.

use diagnostic_ext::{clear_diagnostics, process_diagnostics, publish_diagnostics};
use hover::try_into_hover_result;
use jump_definition::get_definition_span;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::{jsonrpc::Error, lsp_types::*, Client, LanguageServer, LspService, Server};
use utils::{convert_slice_url_to_uri, url_to_file_path, FindFile};

mod configuration_set;
mod diagnostic_ext;
mod hover;
mod jump_definition;
mod session;
mod slice_config;
mod utils;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

struct Backend {
    client: Client,
    session: Arc<crate::session::Session>,
}

impl Backend {
    pub fn new(client: tower_lsp::Client) -> Self {
        let session = Arc::new(crate::session::Session::new(client.clone()));
        Self { client, session }
    }

    fn capabilities() -> ServerCapabilities {
        let definition_provider = Some(OneOf::Left(true));
        let hover_provider = Some(HoverProviderCapability::Simple(true));

        let text_document_sync = Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(false),
                })),
                ..Default::default()
            },
        ));

        let workspace = Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            ..Default::default()
        });

        ServerCapabilities {
            text_document_sync,
            workspace,
            definition_provider,
            hover_provider,
            ..Default::default()
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        self.session.update_from_initialize_params(params).await;
        let capabilities = Backend::capabilities();

        Ok(InitializeResult {
            capabilities,
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // When the configuration changes, any of the files in the workspace could be impacted. Therefore, we need to
        // clear the diagnostics for all files and then re-publish them.
        clear_diagnostics(&self.client, &self.session.configuration_sets).await;

        // Update the configuration sets by fetching the configurations from the client. This is performed after
        // initialization because the client may not be ready to provide configurations before initialization.
        self.session.fetch_configurations().await;

        // Publish the diagnostics for all files
        publish_diagnostics(&self.client, &self.session.configuration_sets).await;
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
        clear_diagnostics(&self.client, &self.session.configuration_sets).await;

        // Update the stored configuration sets from the data provided in the client notification
        self.session.update_configurations_from(params).await;

        // Publish the diagnostics for all files
        publish_diagnostics(&self.client, &self.session.configuration_sets).await;
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
        let configuration_sets = self.session.configuration_sets.lock().await;
        let compilation_state = configuration_sets.iter().find_file(&file_name);

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
        let configuration_sets = self.session.configuration_sets.lock().await;
        configuration_sets
            .iter()
            .find_file(&file_name)
            .map(|compilation_state| try_into_hover_result(compilation_state, url, position))
            .transpose()
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
        let mut configuration_sets = self.session.configuration_sets.lock().await;

        // Update the compilation state for the any impacted configuration set
        configuration_sets
            .iter_mut()
            .filter(|config| {
                // Find the configuration set that matches the current configuration
                let files = config
                    .compilation_state
                    .files
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();
                files.contains(&file_name.to_owned())
            })
            .for_each(|configuration_set| {
                // Update the value of the compilation state in the configuration set in the HashMap
                (configuration_set).compilation_state = slicec::compile_from_options(
                    configuration_set.slice_config.as_slice_options(),
                    |_| {},
                    |_| {},
                );
            });

        // Collect the diagnostics for each configuration set
        let diagnostic_sets = configuration_sets
            .iter_mut()
            .filter(|config| {
                // Find the configuration set that matches the current configuration
                let files = config
                    .compilation_state
                    .files
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>();
                files.contains(&file_name.to_owned())
            })
            .map(|configuration_set| {
                let compilation_state = &mut configuration_set.compilation_state;

                // Find the diagnostics for the config set
                let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
                    &compilation_state.ast,
                    &compilation_state.files,
                    configuration_set.slice_config.as_slice_options(),
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
}
