// Copyright (c) ZeroC, Inc.

use crate::configuration_set::ConfigurationSet;
use crate::slice_config::ServerConfig;
use crate::utils::{sanitize_path, url_to_sanitized_file_path};
use tokio::sync::{Mutex, RwLock};
use tower_lsp::lsp_types::DidChangeConfigurationParams;

#[derive(Debug, Default)]
pub struct Session {
    /// This vector contains all of the configuration sets for the language server. Each element is a tuple containing
    /// `SliceConfig` and `CompilationState`. The `SliceConfig` is used to determine which configuration set to use when
    /// publishing diagnostics. The `CompilationState` is used to retrieve the diagnostics for a given file.
    pub configuration_sets: Mutex<Vec<ConfigurationSet>>,
    /// Configuration that affects the entire server.
    pub server_config: RwLock<ServerConfig>,
}

impl Session {
    // Update the properties of the session from `InitializeParams`
    pub async fn update_from_initialize_params(
        &self,
        params: tower_lsp::lsp_types::InitializeParams,
    ) {
        let initialization_options = params.initialization_options;

        // Use the root_uri if it exists temporarily as we cannot access configuration until
        // after initialization. Additionally, LSP may provide the windows path with escaping or a lowercase
        // drive letter. To fix this, we convert the path to a URL and then back to a path.
        let workspace_root_path = params
            .root_uri
            .and_then(|uri| url_to_sanitized_file_path(&uri))
            .map(|path| path.display().to_string())
            .expect("`root_uri` was not sent by the client, or was malformed");

        // This is the path to the built-in Slice files that are included with the extension. It should always
        // be present.
        let built_in_slice_path = initialization_options
            .as_ref()
            .and_then(|opts| opts.get("builtInSlicePath"))
            .and_then(|value| value.as_str())
            .map(sanitize_path)
            .expect("builtInSlicePath not found in initialization options");

        *self.server_config.write().await = ServerConfig { workspace_root_path, built_in_slice_path };

        // Load any user configuration from the 'slice.configurations' option.
        let configuration_sets = initialization_options
            .as_ref()
            .and_then(|opts| opts.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|arr| ConfigurationSet::parse_configuration_sets(arr))
            .unwrap_or_default();

        self.update_configurations(configuration_sets).await;
    }

    // Update the configuration sets from the `DidChangeConfigurationParams` notification.
    pub async fn update_configurations_from_params(&self, params: DidChangeConfigurationParams) {
        // Parse the configurations from the notification
        let configurations = params
            .settings
            .get("slice")
            .and_then(|v| v.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|arr| ConfigurationSet::parse_configuration_sets(arr))
            .unwrap_or_default();

        // Update the configuration sets
        self.update_configurations(configurations).await;
    }

    // Update the configuration sets by replacing it with the new configurations. If there are no configuration sets
    // after updating, insert the default configuration set.
    async fn update_configurations(&self, mut configurations: Vec<ConfigurationSet>) {
        // Insert the default configuration set if needed
        if configurations.is_empty() {
            configurations.push(ConfigurationSet::default());
        }

        *self.configuration_sets.lock().await = configurations;
    }
}
