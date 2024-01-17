// Copyright (c) ZeroC, Inc.

use std::path::{Path, PathBuf};
use slicec::slice_options::SliceOptions;

// This struct holds the configuration for a single compilation set.
#[derive(Default, Debug)]
pub struct SliceConfig {
    paths: Vec<String>,
    workspace_root_path: Option<PathBuf>,
    built_in_slice_path: Option<String>,
    cached_slice_options: SliceOptions,
}

impl SliceConfig {
    // `path` must be absolute.
    pub fn set_workspace_root_path(&mut self, path: PathBuf) {
        self.workspace_root_path = Some(path);
        self.refresh_paths();
    }

    // `path` must be absolute.
    pub fn set_built_in_slice_path(&mut self, path: Option<String>) {
        self.built_in_slice_path = path;
        self.refresh_paths();
    }

    pub fn set_search_paths(&mut self, paths: Vec<String>) {
        self.paths = paths;
        self.refresh_paths();
    }

    // Resolve path URIs to file paths to be used by the Slice compiler.
    fn resolve_paths(&self) -> Vec<String> {
        // If `root_path` isn't set, relative path resolution is impossible, so we return.
        let Some(root_path) = &self.workspace_root_path else {
            return vec![];
        };

        let mut resolved_paths = Vec::new();
        for string_path in &self.paths {
            let path = Path::new(string_path);

            // If the path is absolute, add it as-is. Otherwise, preface it with the workspace root.
            let absolute_path = match path.is_absolute() {
                true => path.to_owned(),
                false => root_path.join(path),
            };
            resolved_paths.push(absolute_path.display().to_string());
        }

        // If the user didn't specify any paths, default to using the workspace root.
        if resolved_paths.is_empty() {
            resolved_paths.push(root_path.display().to_string());
        }

        // Add the built-in Slice files (WellKnownTypes, etc.) to the end of the list if it's present.
        if let Some(path) = &self.built_in_slice_path {
            resolved_paths.push(path.clone());
        }

        resolved_paths
    }

    pub fn as_slice_options(&self) -> &SliceOptions {
        &self.cached_slice_options
    }

    // This function should be called whenever the configuration changes.
    fn refresh_paths(&mut self) {
        self.cached_slice_options.references = self.resolve_paths();
    }
}
