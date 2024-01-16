// Copyright (c) ZeroC, Inc.

use diagnostic_ext::{clear_diagnostics, process_diagnostics, publish_diagnostics};
use hover::try_into_hover_result;
use jump_definition::get_definition_span;
use std::collections::HashMap;
use std::path::Path;
use tower_lsp::{jsonrpc::Error, lsp_types::*, Client, LanguageServer, LspService, Server};
use utils::{convert_slice_url_to_uri, url_to_file_path, FindFile};

use crate::session::Session;

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
    session: Session,
}

impl Backend {
    pub fn new(client: tower_lsp::Client) -> Self {
        let session = Session::new();
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
        // Now that the server and client are fully initialized, it's safe to publish any diagnostics we've found.
        publish_diagnostics(&self.client, &self.session.configuration_sets).await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client
            .log_message(MessageType::INFO, "Extension settings changed")
            .await;

        // When the configuration changes, any of the files in the workspace could be impacted. Therefore, we need to
        // clear the diagnostics for all files and then re-publish them.
        clear_diagnostics(&self.client, &self.session.configuration_sets).await;

        // Update the stored configuration sets from the data provided in the client notification
        self.session.update_configurations_from_params(params).await;

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
        Ok(configuration_sets
            .iter()
            .find_file(&file_name)
            .and_then(|compilation_state| {
                try_into_hover_result(compilation_state, url, position).ok()
            }))
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
            .log_message(MessageType::INFO, format!("File '{file_name}' changed"))
            .await;

        let mut configuration_sets = self.session.configuration_sets.lock().await;
        let mut publish_map = HashMap::new();
        let mut diagnostics = Vec::new();

        // Process each configuration set that contains the changed file
        for set in configuration_sets.iter_mut().filter(|set| {
            set.compilation_state.files.keys().any(|f| {
                let key_path = Path::new(f);
                let file_path = Path::new(file_name);
                key_path == file_path || file_path.starts_with(key_path)
            })
        }) {
            // Update the compilation state of the configuration set
            let slice_options = set.slice_config.as_slice_options();
            set.compilation_state = slicec::compile_from_options(slice_options, |_| {}, |_| {});

            // Collect the diagnostics of the compilation state
            diagnostics.extend(
                std::mem::take(&mut set.compilation_state.diagnostics).into_updated(
                    &set.compilation_state.ast,
                    &set.compilation_state.files,
                    slice_options,
                ),
            );

            // Update publish_map with files to be updated
            publish_map.extend(
                set.compilation_state
                    .files
                    .keys()
                    .filter_map(|uri| convert_slice_url_to_uri(uri))
                    .map(|uri| (uri, vec![])),
            );
        }

        // If there are multiple diagnostics for the same span, that have the same message, deduplicate them
        diagnostics.dedup_by(|d1, d2| d1.span() == d2.span() && d1.message() == d2.message());

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
