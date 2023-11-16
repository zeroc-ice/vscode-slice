// Copyright (c) ZeroC, Inc.

use crate::Backend;
use tower_lsp::lsp_types::{ConfigurationItem, DidChangeConfigurationParams, Url};

#[derive(Default, Debug)]
pub struct SliceConfig {
    pub reference_urls: Option<Vec<Url>>,
}

impl SliceConfig {
    // Attempt to create a SliceConfig from the backend with the provided workspace root.
    pub async fn try_from_backend(
        backend: &Backend,
        workspace_root: &Url,
    ) -> tower_lsp::jsonrpc::Result<SliceConfig> {
        let reference_directories = Self::fetch_reference_directories(backend).await?;
        Self::create_config_from_directories(reference_directories, workspace_root)
    }

    // Creates a SliceConfig from the provided configuration parameters and workspace root.
    pub fn from_configuration_parameters(
        params: DidChangeConfigurationParams,
        workspace_root: &Url,
    ) -> Self {
        let reference_directories = Self::parse_reference_directories(&params);
        Self::create_config_from_directories(reference_directories, workspace_root)
            .unwrap_or_default()
    }

    // Fetch reference directories from the backend.
    async fn fetch_reference_directories(
        backend: &Backend,
    ) -> tower_lsp::jsonrpc::Result<Option<Vec<String>>> {
        let params = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("slice.referenceDirectory".to_string()),
        }];

        Ok(backend
            .client
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
            .and_then(|v| v.get("referenceDirectory"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<String>>()
            })
    }

    // Create a SliceConfig from the provided reference directories and workspace root.
    fn create_config_from_directories(
        reference_directories: Option<Vec<String>>,
        workspace_root: &Url,
    ) -> Result<SliceConfig, tower_lsp::jsonrpc::Error> {
        match reference_directories {
            Some(dirs) => {
                let reference_urls = Self::try_get_reference_urls(dirs, workspace_root)?;
                Ok(SliceConfig {
                    reference_urls: Some(reference_urls),
                })
            }
            None => Ok(SliceConfig::default()),
        }
    }

    // Convert reference directory strings into URLs.
    fn try_get_reference_urls(
        reference_directories: Vec<String>,
        workspace_root: &Url,
    ) -> Result<Vec<Url>, tower_lsp::jsonrpc::Error> {
        let workspace_path = workspace_root.to_file_path().map_err(|_| {
            tower_lsp::jsonrpc::Error::invalid_params(
                "Failed to convert workspace URL to file path.",
            )
        })?;

        reference_directories
            .iter()
            .map(|dir| {
                let path = workspace_path.join(dir);
                Url::from_file_path(path).map_err(|_| {
                    tower_lsp::jsonrpc::Error::invalid_params(
                        "Failed to convert reference path to URL.",
                    )
                })
            })
            .collect()
    }

    // Resolve reference URIs to file paths
    pub fn resolve_reference_paths(&self, workspace_root: Option<&Url>) -> Vec<String> {
        let workspace_path = match workspace_root {
            Some(url) => match url.to_file_path() {
                Ok(path) => Some(path),
                Err(_) => return Vec::new(), // Handle error or return empty vector
            },
            None => None,
        };
        self.reference_urls
            .as_ref()
            .map(|uris| {
                uris.iter()
                    .filter_map(|uri| {
                        uri.to_file_path().ok().and_then(|path| {
                            if path.is_absolute() {
                                Some(path.display().to_string())
                            } else {
                                workspace_path.as_ref().map(|workspace_path| {
                                    workspace_path.join(&path).display().to_string()
                                })
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
