// Copyright (c) ZeroC, Inc.

use std::path::PathBuf;

use slicec::slice_options::SliceOptions;

/// This struct holds configuration that affects the entire server.
#[derive(Debug, Default)]
pub struct ServerConfig {
    /// This is the root path of the workspace, used to resolve relative paths. It must be an absolute path.
    pub workspace_root_path: PathBuf,
    /// This is the path to the built-in Slice files that are included with the extension. It must be an absolute path.
    pub built_in_slice_path: String,
}

/// This struct holds the configuration for a single compilation set.
#[derive(Debug)]
pub struct SliceConfig {
    /// List of paths that will be passed to the compiler as reference files/directories.
    pub slice_search_paths: Vec<PathBuf>,
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

pub fn compute_slice_options(server_config: &ServerConfig, set_config: &SliceConfig) -> SliceOptions {
    let root_path = &server_config.workspace_root_path;
    let mut slice_options = SliceOptions::default();
    let references = &mut slice_options.references;

    // Add the built-in Slice files (WellKnownTypes, etc.) at the start of the list, if they should be included.
    // Putting them first ensures that any redefinition conflicts will appear in the user's files, and not these.
    // (Since `slicec` parses files in the order that they are provided).
    if set_config.include_built_in_slice_files {
        references.push(server_config.built_in_slice_path.clone());
    }

    match set_config.slice_search_paths.as_slice() {
        // If the user didn't specify any paths, default to using the workspace root.
        [] => references.push(root_path.display().to_string()),

        // Otherwise, add in the user-specified search paths.
        user_paths => {
            for path in user_paths {
                // If the path is absolute, add it as-is. Otherwise, preface it with the workspace root.
                let absolute_path = match path.is_absolute() {
                    true => path.to_owned(),
                    false => root_path.join(path),
                };
                references.push(absolute_path.display().to_string());
            }
        }
    }

    slice_options
}
