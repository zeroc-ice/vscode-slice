// Copyright (c) ZeroC, Inc.

use crate::configuration::ServerConfig;
use crate::slice_project::SliceProject;
use crate::utils::{sanitize_path, url_to_sanitized_file_path};
use tower_lsp::lsp_types::{DidChangeConfigurationParams, InitializeParams};

#[derive(Debug, Default)]
pub struct ServerState {
    /// This vector contains all of the Slice projects for the language server. Each element is a tuple containing
    /// `ProjectConfig` and `CompilationState`. The `ProjectConfig` is used to determine which project to use
    /// when publishing diagnostics. The `CompilationState` is used to retrieve the diagnostics for a given file.
    pub slice_projects: Vec<SliceProject>,
    /// Configuration that affects the entire server.
    pub server_config: ServerConfig,
}

impl ServerState {
    // Update the properties of the server from `InitializeParams`
    pub fn update_from_initialize_params(&mut self, params: InitializeParams) {
        let initialization_options = params.initialization_options;

        // Use the root_uri if it exists temporarily as we cannot access configuration until
        // after initialization. Additionally, LSP may provide the windows path with escaping or a lowercase
        // drive letter. To fix this, we convert the path to a URL and then back to a path.
        let workspace_root_path = params
            .root_uri
            .and_then(|uri| url_to_sanitized_file_path(&uri))
            .expect("`root_uri` was not sent by the client, or was malformed");

        // This is the path to the built-in Slice files that are included with the extension. It should always
        // be present.
        let built_in_slice_path = initialization_options
            .as_ref()
            .and_then(|opts| opts.get("builtInSlicePath"))
            .and_then(|value| value.as_str())
            .map(sanitize_path)
            .expect("builtInSlicePath not found in initialization options");

        self.server_config = ServerConfig { workspace_root_path, built_in_slice_path };

        // Load the active Slice projects from the 'slice.configurations' option.
        let slice_projects = initialization_options
            .as_ref()
            .and_then(|opts| opts.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|arr| SliceProject::parse_slice_projects(arr))
            .unwrap_or_default();

        self.set_projects(slice_projects);
    }

    // Update the active projects from the `DidChangeConfigurationParams` notification.
    pub fn update_projects_from_params(&mut self, params: DidChangeConfigurationParams) {
        // Parse the projects from the notification
        let slice_projects = params
            .settings
            .get("slice")
            .and_then(|v| v.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|arr| SliceProject::parse_slice_projects(arr))
            .unwrap_or_default();

        self.set_projects(slice_projects);
    }

    // Sets the currently loaded Slice projects.
    // If no Slice projects are provided, we insert a default project.
    fn set_projects(&mut self, mut projects: Vec<SliceProject>) {
        if projects.is_empty() {
            projects.push(SliceProject::default());
        }

        self.slice_projects = projects;
    }
}
