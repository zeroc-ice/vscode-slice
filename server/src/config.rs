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
    pub fn try_update_from_params(&mut self, params: &DidChangeConfigurationParams) {
        self.references = Self::parse_reference_directories(params);
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

    // Convert reference directory strings into URLs.
    fn try_get_reference_urls(&self) -> Result<Vec<Url>, tower_lsp::jsonrpc::Error> {
        // Convert the root_uri to a file path
        let root_uri_error =
            || tower_lsp::jsonrpc::Error::invalid_params("Failed to process URL or file path.");
        let root_path = self
            .root_uri
            .as_ref()
            .ok_or_else(root_uri_error)?
            .to_file_path()
            .map_err(|_| root_uri_error())?;

        // Convert reference directories to URLs or use root_uri if none are present
        let result_urls = match self.references.as_ref() {
            Some(dirs) => dirs
                .iter()
                .map(|dir| {
                    Url::from_file_path(root_path.join(dir)).map_err(|_| {
                        tower_lsp::jsonrpc::Error::invalid_params(
                            "Failed to convert reference path to URL.",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => vec![self.root_uri.clone().ok_or_else(|| {
                tower_lsp::jsonrpc::Error::invalid_params("Root URI is not set.")
            })?],
        };

        Ok(result_urls)
    }

    // Resolve reference URIs to file paths to be used by the Slice compiler.
    pub fn resolve_reference_paths(&self) -> Vec<String> {
        let reference_urls: Vec<Url> = self.try_get_reference_urls().unwrap_or_default();

        // If no reference directories are set, use the root_uri as the reference directory.
        if reference_urls.is_empty() {
            return self
                .root_uri
                .as_ref()
                .and_then(|url| url.to_file_path().ok())
                .map(|path| vec![path.display().to_string()])
                .unwrap_or_default();
        }

        // Convert reference URLs to file paths
        reference_urls
            .iter()
            .filter_map(|uri| {
                let path = uri.to_file_path().ok()?;
                if path.is_absolute() {
                    Some(path.display().to_string())
                } else {
                    self.root_uri
                        .as_ref()?
                        .to_file_path()
                        .ok()
                        .map(|root_path| root_path.join(&path).display().to_string())
                }
            })
            .collect()
    }
}
