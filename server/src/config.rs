// Copyright (c) ZeroC, Inc.

use tower_lsp::{
    lsp_types::{ConfigurationItem, DidChangeConfigurationParams, Url},
    Client,
};

#[derive(Default, Debug)]
pub struct SliceConfig {
    pub references: Option<Vec<String>>,
    pub root_uri: Option<Url>,
}

impl SliceConfig {
    pub async fn try_update_from_params(
        &mut self,
        params: &DidChangeConfigurationParams,
    ) -> tower_lsp::jsonrpc::Result<()> {
        self.references = Self::parse_reference_directories(params);
        Ok(())
    }

    pub async fn try_update_from_client(
        &mut self,
        client: &Client,
    ) -> tower_lsp::jsonrpc::Result<()> {
        self.references = Self::fetch_reference_directories(client).await?;
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

    // Resolve reference URIs to file paths to be used by the Slice compiler.
    pub fn resolve_reference_paths(&self) -> Vec<String> {
        // If `root_uri` isn't set, or doesn't represent a valid file path, path resolution is impossible, so we return.
        let Some(root_uri) = &self.root_uri else {
            return vec![];
        };
        let Ok(root_path) = root_uri.to_file_path() else {
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
        if result_urls.is_empty() {
            vec![root_path.display().to_string()]
        } else {
            result_urls
        }
    }
}
