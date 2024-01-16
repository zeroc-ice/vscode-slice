// Copyright (c) ZeroC, Inc.

use slicec::slice_options::SliceOptions;
use tower_lsp::lsp_types::Url;

// This struct holds the configuration for a single compilation set.
#[derive(Default, Debug)]
pub struct SliceConfig {
    paths: Vec<String>,
    root_uri: Option<Url>,
    include_built_in_path: bool,
    built_in_slice_path: String,
    cached_slice_options: SliceOptions,
}

impl SliceConfig {
    // `root` must be absolute.
    pub fn set_root_uri(&mut self, root: Url) {
        self.root_uri = Some(root);
        self.refresh_paths();
    }

    // `path` must be absolute.
    pub fn set_built_in_path(&mut self, path: String) {
        self.built_in_slice_path = path;
        self.refresh_paths();
    }

    pub fn update_from_paths(&mut self, paths: Vec<String>) {
        self.paths = paths;
        self.refresh_paths();
    }

    pub fn update_include_built_in_path(&mut self, include: bool) {
        self.include_built_in_path = include;
        self.refresh_paths();
    }

    // Resolve path URIs to file paths to be used by the Slice compiler.
    fn resolve_paths(&self) -> Vec<String> {
        // If `root_uri` isn't set, or doesn't represent a valid file path, path resolution is impossible, so we return.
        let Some(Ok(root_path)) = self.root_uri.as_ref().map(Url::to_file_path) else {
            return vec![];
        };

        // Convert path directories to URLs.
        let mut result_urls = Vec::new();

        for path in &self.paths {
            match Url::from_file_path(root_path.join(path)) {
                Ok(path_url) => {
                    // If this url doesn't represent a valid file path, skip it.
                    let Ok(file_path) = path_url.to_file_path() else {
                        continue;
                    };

                    // If the path is absolute, add it as-is. Otherwise, preface it with the workspace root.
                    if file_path.is_absolute() {
                        result_urls.push(file_path.display().to_string());
                    } else {
                        let other_path = root_path.join(&file_path);
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
        if self.include_built_in_path && !self.built_in_slice_path.is_empty() {
            paths.push(self.built_in_slice_path.clone());
        }
        paths
    }

    pub fn as_slice_options(&self) -> &SliceOptions {
        &self.cached_slice_options
    }

    // This function should be called whenever the configuration changes.
    fn refresh_paths(&mut self) {
        self.cached_slice_options.references = self.resolve_paths();
    }
}
