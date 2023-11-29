// Copyright (c) ZeroC, Inc.

use slicec::slice_options::SliceOptions;
use tower_lsp::{
    lsp_types::{ConfigurationItem, DidChangeConfigurationParams, Url},
    Client,
};

#[derive(Default, Debug)]
pub struct SliceConfig {
    built_in_slice_path: String,
    references: Option<Vec<String>>,
    root_uri: Option<Url>,
    cached_slice_options: SliceOptions,
}

impl SliceConfig {
    pub fn set_root_uri(&mut self, root: Url) {
        self.root_uri = Some(root);
        self.refresh_reference_paths();
    }

    pub fn set_built_in_reference(&mut self, path: impl Into<String>) {
        self.built_in_slice_path = path.into();
    }

    pub fn update_from_params(&mut self, params: &DidChangeConfigurationParams) {
        self.references = Self::parse_reference_directories(params);
        self.refresh_reference_paths();
    }

    pub async fn try_update_from_client(
        &mut self,
        client: &Client,
    ) -> tower_lsp::jsonrpc::Result<()> {
        self.references = Self::fetch_reference_directories(client).await?;
        self.refresh_reference_paths();
        Ok(())
    }

    // Fetch reference directories from the backend.
    async fn fetch_reference_directories(
        client: &Client,
    ) -> tower_lsp::jsonrpc::Result<Option<Vec<String>>> {
        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.referenceDirectories".to_string()),
        }];

        Ok(client
            .configuration(params)
            .await?
            .first()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            }))
    }

    // Parse reference directories from configuration parameters.
    fn parse_reference_directories(params: &DidChangeConfigurationParams) -> Option<Vec<String>> {
        params
            .settings
            .get("slice")
            .and_then(|v| v.get("referenceDirectories"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<String>>()
            })
    }

    /// Resolve and cache reference URIs to file paths to be used by the Slice compiler.
    /// This function should be called whenever the configuration changes.
    fn refresh_reference_paths(&mut self) {
        self.cached_slice_options.references = self.resolve_reference_paths();
    }

    // Resolve reference URIs to file paths to be used by the Slice compiler.
    fn resolve_reference_paths(&self) -> Vec<String> {
        // If `root_uri` isn't set, or doesn't represent a valid file path, path resolution is impossible, so we return.
        let Some(Ok(root_path)) = self.root_uri.as_ref().map(Url::to_file_path) else {
            return vec![];
        };

        // If no references are set, default to using the workspace root.
        let Some(references) = &self.references else {
            let default_path = match root_path.is_absolute() {
                true => root_path.display().to_string(),
                false => root_path.join(&root_path).display().to_string(),
            };
            return vec![default_path];
        };

        // Convert reference directories to URLs.
        let mut result_urls = Vec::new();
        for reference in references {
            match Url::from_file_path(root_path.join(reference)) {
                Ok(reference_url) => {
                    // If this url doesn't represent a valid file path, skip it.
                    let Ok(absolute_path) = reference_url.to_file_path() else {
                        continue;
                    };

                    // If the path is absolute, add it as-is. Otherwise, preface it with the workspace root.
                    if absolute_path.is_absolute() {
                        result_urls.push(absolute_path.display().to_string());
                    } else {
                        let other_path = root_path.join(&absolute_path);
                        result_urls.push(other_path.display().to_string());
                    }
                }
                Err(_) => return vec![root_path.display().to_string()],
            }
        }

        // If references was set to an empty list, or none of them represented a valid directory path, default to using
        // the workspace root. Otherwise, if there's more than zero valid reference directories, return them.
        let mut paths = if result_urls.is_empty() {
            vec![root_path.display().to_string()]
        } else {
            result_urls
        };
        // Add the built-in reference path to the end of the list.
        paths.push(self.built_in_slice_path.clone());
        paths
    }

    pub fn as_slice_options(&self) -> &SliceOptions {
        &self.cached_slice_options
    }
}
