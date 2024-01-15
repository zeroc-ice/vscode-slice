// Copyright (c) ZeroC, Inc.

use crate::configuration_set::ConfigurationSet;
use tokio::sync::Mutex;
use tower_lsp::{
    lsp_types::{ConfigurationItem, DidChangeConfigurationParams},
    Client,
};

pub struct Session {
    /// This vector contains all of the configuration sets for the language server. Each element is a tuple containing
    /// `SliceConfig` and `CompilationState`. The `SliceConfig` is used to determine which configuration set to use when
    /// publishing diagnostics. The `CompilationState` is used to retrieve the diagnostics for a given file.
    pub configuration_sets: Mutex<Vec<ConfigurationSet>>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            configuration_sets: Mutex::new(Vec::new()),
        }
    }

    // Update the stored configuration sets by fetching them from the client.
    pub async fn fetch_configurations(&self, client: &Client) {
        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.configurations".to_string()),
        }];

        // Fetch the configurations from the client and parse them.
        let configurations = client
            .configuration(params)
            .await
            .ok()
            .map(|response| {
                let config_array = &response
                    .iter()
                    .filter_map(|config| config.as_array())
                    .flatten()
                    .cloned()
                    .collect::<Vec<_>>();
                ConfigurationSet::parse_configuration_sets(config_array)
            })
            .unwrap_or_default();

        // Update the configuration sets
        self.update_configurations(configurations).await;
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
            configurations.push(ConfigurationSet::new());
        }

        let mut configuration_sets = self.configuration_sets.lock().await;
        *configuration_sets = configurations;
    }
}
