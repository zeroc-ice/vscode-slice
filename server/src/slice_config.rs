// Copyright (c) ZeroC, Inc.

use std::path::{Path, PathBuf};

// This struct holds the configuration for a single compilation set.
#[derive(Debug)]
pub struct SliceConfig {
    pub paths: Vec<String>,
    pub workspace_root_path: Option<PathBuf>,
    pub built_in_slice_path: Option<String>,
}

impl SliceConfig {
    // Resolve path URIs to file paths to be used by the Slice compiler.
    pub fn resolve_paths(&self) -> Vec<String> {
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
}
