// Copyright (c) ZeroC, Inc.

use slicec::slice_options::SliceOptions;
use tower_lsp::lsp_types::Url;

#[derive(Debug)]
pub struct ServerConfig {
    /// This is the root URI of the workspace. It is used to resolve relative paths in the configuration.
    pub root_uri: Url,
    /// This is the path to the built-in Slice files that are included with the extension.
    pub built_in_slice_path: String,
}

// This struct holds the configuration for a single compilation set.
#[derive(Default, Debug)]
pub struct SliceConfig {
    paths: Option<Vec<String>>,
    include_built_in_path: bool,
    cached_slice_options: Option<SliceOptions>,
}

impl SliceConfig {
    pub fn update_from_paths(&mut self, paths: Vec<String>) {
        self.paths = Some(paths);
        self.cached_slice_options = None; // Invalidate the cache so it gets regenerated.
    }

    pub fn update_include_built_in_path(&mut self, include: bool) {
        self.include_built_in_path = include;
        self.cached_slice_options = None; // Invalidate the cache so it gets regenerated.
    }

    // Resolve path URIs to file paths to be used by the Slice compiler.
    fn resolve_paths(&self, server_config: &ServerConfig) -> Vec<String> {
        // If `root_uri` doesn't represent a valid file path, path resolution is impossible, so we return.
        let Ok(root_path) = Url::to_file_path(&server_config.root_uri) else {
            return vec![];
        };

        // If no paths are set, default to using the workspace root.
        let Some(paths) = &self.paths else {
            let default_path = match root_path.is_absolute() {
                true => root_path.display().to_string(),
                false => root_path.join(&root_path).display().to_string(),
            };
            return vec![default_path];
        };

        // Convert path directories to URLs.
        let mut result_urls = Vec::new();

        for path in paths {
            match Url::from_file_path(root_path.join(path)) {
                Ok(path_url) => {
                    // If this url doesn't represent a valid file path, skip it.
                    let Ok(absolute_path) = path_url.to_file_path() else {
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

        // If paths was set to an empty list, or none of them represented a valid directory path or file path, default
        // to using the workspace root. Otherwise, if there's more than zero valid paths, return them.
        let mut paths = if result_urls.is_empty() {
            vec![root_path.display().to_string()]
        } else {
            result_urls
        };
        // Add the well known types path to the end of the list.
        // TODO: Weird case where `include_built_in_path` is true but `built_in_slice_path` is empty.
        // We should probably handle this case better or make sure it never happens.
        if self.include_built_in_path && !server_config.built_in_slice_path.is_empty() {
            paths.push(server_config.built_in_slice_path.clone());
        }
        paths
    }

    pub fn as_slice_options(&mut self) -> &SliceOptions {
        if self.cached_slice_options.is_none() {
            let mut slice_options = SliceOptions::default();
            slice_options.references = self.resolve_paths(crate::server_config());
            self.cached_slice_options = Some(slice_options);
        }

        self.cached_slice_options.as_ref().unwrap()
    }
}
