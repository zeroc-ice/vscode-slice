use std::collections::HashMap;

use slicec::compilation_state::CompilationState;
use tokio::sync::{Mutex, RwLock};
use tower_lsp::lsp_types::{ConfigurationItem, DidChangeConfigurationParams, Url};

use crate::{
    config::SliceConfig,
    utils::{new_configuration_set, parse_configuration_sets},
    ConfigurationSet,
};

pub struct Session {
    client: tower_lsp::Client,
    // This HashMap contains all of the configuration sets for the language server. The key is the SliceConfig and the
    // value is the CompilationState. The SliceConfig is used to determine which configuration set to use when
    // publishing diagnostics. The CompilationState is used to retrieve the diagnostics for a given file.
    pub configuration_sets: Mutex<HashMap<SliceConfig, CompilationState>>,
    // This is the root URI of the workspace. It is used to resolve relative paths in the configuration.
    pub root_uri: RwLock<Option<Url>>,
    // This is the path to the built-in Slice files that are included with the extension.
    pub built_in_slice_path: RwLock<String>,
}

impl Session {
    pub fn new(client: tower_lsp::Client) -> Self {
        Self {
            client,
            configuration_sets: Mutex::new(HashMap::new()),
            root_uri: RwLock::new(None),
            built_in_slice_path: RwLock::new(String::new()),
        }
    }

    // Update the properties of the session from `InitializeParams`
    pub async fn update_from_initialize_params(
        &self,
        params: tower_lsp::lsp_types::InitializeParams,
    ) {
        // This is the path to the built-in Slice files that are included with the extension. It should always
        // be present.
        let built_in_slice_path = params
            .initialization_options
            .and_then(|opts| opts.get("builtInSlicePath").cloned())
            .and_then(|v| v.as_str().map(str::to_owned))
            .expect("builtInSlicePath not found in initialization options");
        *self.built_in_slice_path.write().await = built_in_slice_path.clone();

        // Use the root_uri if it exists temporarily as we cannot access configuration until
        // after initialization. Additionally, LSP may provide the windows path with escaping or a lowercase
        // drive letter. To fix this, we convert the path to a URL and then back to a path.
        let root_uri = params
            .root_uri
            .and_then(|uri| uri.to_file_path().ok())
            .and_then(|path| Url::from_file_path(path).ok())
            .expect("root_uri not found in initialization parameters");
        *self.root_uri.write().await = Some(root_uri.clone());

        // // Insert the default configuration set into the HashMap. This will be updated later if the client provides
        // // configurations.
        // let mut configuration_sets = self.configuration_sets.lock().await;
        // let default = new_configuration_set(root_uri, built_in_slice_path);
        // configuration_sets.insert(default.0, default.1);
    }

    // Update the stored configuration sets by fetching them from the client.
    pub async fn fetch_configurations(&self) {
        let built_in_slice_path = &self.built_in_slice_path.read().await;
        let root_uri_guard = self.root_uri.read().await;
        let root_uri = (*root_uri_guard).clone().expect("root_uri not set");

        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.configurations".to_string()),
        }];

        // Fetch the configurations from the client and parse them.
        let configurations = self
            .client
            .configuration(params)
            .await
            .ok()
            .map(|response| {
                parse_configuration_sets(
                    &response
                        .iter()
                        .filter_map(|config| config.as_array())
                        .flatten()
                        .cloned()
                        .collect::<Vec<_>>(),
                    &root_uri,
                    built_in_slice_path,
                )
            })
            .unwrap_or_default();

        // Update the configuration sets
        self.update_configurations(configurations).await;
    }

    // Update the configuration sets from the `DidChangeConfigurationParams` notification.
    pub async fn update_configurations_from(&self, params: DidChangeConfigurationParams) {
        let root_uri = self.root_uri.read().await;
        let built_in_slice_path = &self.built_in_slice_path.read().await;
        let configurations = params
            .settings
            .get("slice")
            .and_then(|v| v.get("configurations"))
            .and_then(|v| v.as_array())
            .map(|config_array| {
                parse_configuration_sets(
                    config_array,
                    &(*root_uri).clone().unwrap(),
                    built_in_slice_path,
                )
            })
            .unwrap_or_default();

        // Update the configuration sets
        self.update_configurations(configurations).await;
    }

    // Update the configuration sets by replacing it with the new configurations. If there are no configuration sets
    // after updating, insert the default configuration set.
    async fn update_configurations(&self, configurations: Vec<ConfigurationSet>) {
        let mut configuration_sets = self.configuration_sets.lock().await;
        *configuration_sets = configurations.into_iter().collect();

        // Insert the default configuration set if needed
        if configuration_sets.is_empty() {
            let root_uri = self.root_uri.read().await;
            let built_in_slice_path = self.built_in_slice_path.read().await;
            let default =
                new_configuration_set(root_uri.clone().unwrap(), built_in_slice_path.clone());
            configuration_sets.insert(default.0, default.1);
        }
    }
}
