// Copyright (c) ZeroC, Inc.

use crate::configuration::compute_slice_options;
use crate::diagnostic_handler::{clear_diagnostics, process_diagnostics, publish_diagnostics_for_project};
use crate::hover::get_hover_message;
use crate::jump_definition::get_definition_span;
use crate::notifications::{ShowNotification, ShowNotificationParams};
use crate::server_state::ServerState;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::path::Path;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Error;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use utils::{convert_slice_path_to_uri, span_to_range, url_to_sanitized_file_path};

mod configuration;
mod diagnostic_handler;
mod hover;
mod jump_definition;
mod notifications;
mod server_state;
mod slice_project;
mod utils;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(SliceLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

struct SliceLanguageServer {
    client_handle: Client,
    server_state: Mutex<ServerState>,
}

impl SliceLanguageServer {
    pub fn new(client_handle: tower_lsp::Client) -> Self {
        let server_state = Mutex::new(ServerState::default());
        Self { client_handle, server_state }
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

    async fn handle_file_change(&self, file_path: &Path) {
        self.client_handle
            .log_message(MessageType::INFO, format!("File '{}' changed", file_path.display()))
            .await;

        let mut server_guard = self.server_state.lock().await;
        let ServerState { slice_projects, server_config } = server_guard.deref_mut();

        let mut publish_map = HashMap::new();
        let mut diagnostics = Vec::new();

        // Process each project that contains the changed file.
        for project in slice_projects.iter_mut().filter(|project| {
            compute_slice_options(server_config, &project.project_config)
                .references
                .into_iter()
                .any(|f| {
                    let key_path = Path::new(&f);
                    key_path == file_path || file_path.starts_with(key_path)
                })
        }) {
            // `trigger_compilation` compiles the project's files and returns any diagnostics.
            diagnostics.extend(project.trigger_compilation(server_config));

            // Update publish_map with files to be updated.
            publish_map.extend(
                project.compilation_data
                    .files
                    .keys()
                    .filter_map(convert_slice_path_to_uri)
                    .map(|uri| (uri, vec![])),
            );
        }

        // If there are multiple diagnostics for the same span, that have the same message, deduplicate them.
        diagnostics.dedup_by(|d1, d2| d1.span() == d2.span() && d1.message() == d2.message());

        // Group the diagnostics by file since diagnostics are published per file and diagnostic.span contains the URL.
        // Process diagnostics and update publish_map.
        // Any diagnostics that do not have a span are returned for further processing.
        let spanless_diagnostics = process_diagnostics(diagnostics, &mut publish_map);
        for diagnostic in spanless_diagnostics {
            show_popup(
                &self.client_handle,
                diagnostic.message(),
                notifications::MessageType::Error,
            )
            .await;
        }

        // Publish the diagnostics for each file.
        self.client_handle
            .log_message(
                MessageType::INFO,
                "Publishing diagnostics for all projects.",
            )
            .await;

        for (uri, lsp_diagnostics) in publish_map {
            self.client_handle
                .publish_diagnostics(uri, lsp_diagnostics, None)
                .await;
        }
    }

    /// Triggers and compilation and publishes any diagnostics that are reported.
    /// It does this for all projects.
    pub async fn compile_and_publish_diagnostics(&self) {
        let mut server_guard = self.server_state.lock().await;
        let ServerState { slice_projects, server_config } = server_guard.deref_mut();

        self.client_handle
            .log_message(
                MessageType::INFO,
                "Publishing diagnostics for all projects.",
            )
            .await;
        for project in slice_projects.iter_mut() {
            // Trigger a compilation and get any diagnostics that were reported during it.
            let diagnostics = project.trigger_compilation(server_config);
            // Publish those diagnostics.
            publish_diagnostics_for_project(&self.client_handle, diagnostics, project).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for SliceLanguageServer {
    async fn initialize(&self, params: InitializeParams) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        let mut server_guard = self.server_state.lock().await;
        server_guard.update_from_initialize_params(params);

        let capabilities = SliceLanguageServer::capabilities();
        Ok(InitializeResult {
            capabilities,
            ..InitializeResult::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Now that the server and client are fully initialized, it's safe to compile and publish any diagnostics.
        self.compile_and_publish_diagnostics().await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        self.client_handle
            .log_message(MessageType::INFO, "Extension settings changed")
            .await;

        // Explicit scope to ensure the server state lock guard is dropped before we start compilation.
        {
            let mut server_guard = self.server_state.lock().await;

            // When the configuration changes, any of the files in the workspace could be impacted.
            // Therefore, we need to clear the diagnostics for all files and then re-publish them.
            clear_diagnostics(&self.client_handle, &server_guard.slice_projects).await;

            // Update the stored Slice projects from the data provided in the client notification.
            server_guard.update_projects_from_params(params);
        }

        // Trigger a compilation and publish the diagnostics for all files.
        self.compile_and_publish_diagnostics().await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Convert the URI to a file path and back to a URL to ensure that the URI is formatted correctly for Windows.
        let file_path = url_to_sanitized_file_path(&uri).ok_or_else(Error::internal_error)?;

        // Find the project that contains the file
        let server_guard = self.server_state.lock().await;
        let slice_projects = &server_guard.slice_projects;

        // Get the definition span and convert it to a GotoDefinitionResponse
        Ok(slice_projects.iter().find_map(|project| {
            let files = &project.compilation_data.files;
            files
                .get(&file_path)
                .and_then(|file| get_definition_span(file, position))
                .map(|location| {
                    GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range: span_to_range(location),
                    })
                })
        }))
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;

        // Convert the URI to a file path and back to a URL to ensure that the URI is formatted correctly for Windows.
        let file_path = url_to_sanitized_file_path(&uri).ok_or_else(Error::internal_error)?;

        // Find the project that contains the file and get the hover info
        let server_guard = self.server_state.lock().await;
        let slice_projects = &server_guard.slice_projects;

        Ok(slice_projects.iter().find_map(|project| {
            let files = &project.compilation_data.files;
            files
                .get(&file_path)
                .and_then(|file| get_hover_message(file, position))
                .map(|message| Hover {
                    contents: HoverContents::Scalar(MarkedString::String(message)),
                    range: None,
                })
        }))
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        if let Some(file_path) = url_to_sanitized_file_path(&params.text_document.uri) {
            self.handle_file_change(&file_path).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(file_path) = url_to_sanitized_file_path(&params.text_document.uri) {
            self.handle_file_change(&file_path).await;
        }
    }
}

pub async fn show_popup(client_handle: &Client, message: String, message_type: notifications::MessageType) {
    let show_notification_params = ShowNotificationParams { message, message_type };
    client_handle
        .send_notification::<ShowNotification>(show_notification_params)
        .await;
}
