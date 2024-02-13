// Copyright (c) ZeroC, Inc.

use std::path::Path;

use slicec::slice_options::SliceOptions;

/// This struct holds configuration that affects the entire server.
#[derive(Debug, Default)]
pub struct ServerConfig {
    /// This is the root path of the workspace, used to resolve relative paths. It must be an absolute path.
    pub workspace_root_path: String,
    /// This is the path to the built-in Slice files that are included with the extension. It must be an absolute path.
    pub built_in_slice_path: String,
}

/// This struct holds the configuration for a single compilation set.
#[derive(Debug)]
pub struct SliceConfig {
    /// List of paths that will be passed to the compiler as reference files/directories.
    pub slice_search_paths: Vec<String>,
    /// Specifies whether to include the built-in Slice files that are bundled with the extension.
    pub include_built_in_slice_files: bool,
}

impl Default for SliceConfig {
    fn default() -> Self {
        SliceConfig {
            slice_search_paths: vec![],
            include_built_in_slice_files: true,
        }
    }
}

pub fn compute_slice_options(server_config: &ServerConfig, unit_config: &SliceConfig) -> SliceOptions {
    let root_path = Path::new(&server_config.workspace_root_path);
    let mut slice_options = SliceOptions::default();
    let references = &mut slice_options.references;

    // Add any user specified search paths at the front of the list.
    for string_path in &unit_config.slice_search_paths {
        let path = Path::new(string_path);

        // If the path is absolute, add it as-is. Otherwise, preface it with the workspace root.
        let absolute_path = match path.is_absolute() {
            true => path.to_owned(),
            false => root_path.join(path),
        };
        references.push(absolute_path.display().to_string());
    }

    // If the user didn't specify any paths, default to using the workspace root.
    if references.is_empty() {
        references.push(root_path.display().to_string());
    }

    // Add the built-in Slice files (WellKnownTypes, etc.) to the end of the list, if they should be included.
    if unit_config.include_built_in_slice_files {
        references.push(server_config.built_in_slice_path.clone());
    }

    slice_options
}
