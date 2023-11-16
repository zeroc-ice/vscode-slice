// Copyright (c) ZeroC, Inc.

use crate::Backend;
use tower_lsp::lsp_types::{ConfigurationItem, DidChangeConfigurationParams, Url};

#[derive(Default, Debug)]
pub struct SliceConfig {
    pub reference_urls: Option<Vec<Url>>,
}

impl SliceConfig {
    // Attempt to create a SliceConfig from the backend with the provided uri root.
    pub async fn try_from_backend(
        backend: &Backend,
        root_uri: &Url,
    ) -> tower_lsp::jsonrpc::Result<SliceConfig> {
        let reference_directories = Self::fetch_reference_directories(backend).await?;
        Self::create_config_from_directories(reference_directories, root_uri)
    }

    // Creates a SliceConfig from the provided configuration parameters and uri root.
    pub fn from_configuration_parameters(
        params: DidChangeConfigurationParams,
        root_uri: &Url,
    ) -> Self {
        let reference_directories = Self::parse_reference_directories(&params);
        Self::create_config_from_directories(reference_directories, root_uri).unwrap_or_default()
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

    // Create a SliceConfig from the provided reference directories and uri_root.
    fn create_config_from_directories(
        reference_directories: Option<Vec<String>>,
        root_uri: &Url,
    ) -> Result<SliceConfig, tower_lsp::jsonrpc::Error> {
        match reference_directories {
            Some(dirs) => {
                let reference_urls = Self::try_get_reference_urls(dirs, root_uri)?;
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
        root_uri: &Url,
    ) -> Result<Vec<Url>, tower_lsp::jsonrpc::Error> {
        let root_path = root_uri.to_file_path().map_err(|_| {
            tower_lsp::jsonrpc::Error::invalid_params("Failed to convert root URL to file path.")
        })?;

        reference_directories
            .iter()
            .map(|dir| {
                let path = root_path.join(dir);
                Url::from_file_path(path).map_err(|_| {
                    tower_lsp::jsonrpc::Error::invalid_params(
                        "Failed to convert reference path to URL.",
                    )
                })
            })
            .collect()
    }

    // Resolve reference URIs to file paths
    pub fn resolve_reference_paths(&self, root_uri: Option<&Url>) -> Vec<String> {
        if self.reference_urls.as_ref().map_or(true, Vec::is_empty) {
            // If self.reference_urls is None or empty, use the root_uri as the reference path.
            return root_uri
                .map(|url| {
                    url.to_file_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default()
                })
                .into_iter()
                .collect();
        }

        self.reference_urls
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|uri| {
                uri.to_file_path().ok().and_then(|path| {
                    if path.is_absolute() {
                        Some(path.display().to_string())
                    } else {
                        root_uri.and_then(|root_url| {
                            root_url
                                .to_file_path()
                                .ok()
                                .map(|root_path| root_path.join(&path).display().to_string())
                        })
                    }
                })
            })
            .collect()
    }
}
