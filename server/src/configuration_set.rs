// Copyright (c) ZeroC, Inc.

use crate::slice_config;
use crate::utils::sanitize_path;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use slice_config::SliceConfig;
use slicec::{ast::Ast, diagnostics::Diagnostic, slice_file::SliceFile};
use slicec::compilation_state::CompilationState;

#[derive(Debug)]
pub struct CompilationData {
    pub ast: Ast,
    pub files: HashMap<String, SliceFile>,
}

impl Default for CompilationData {
    fn default() -> Self {
        Self {
            ast: Ast::create(),
            files: HashMap::default(),
        }
    }
}

// Necessary for using `CompilationData` within async functions.
//
// # Safety
//
// These implementations are safe because `CompilationData` is entirely self-contained and hence can go between threads.
// Note that `files` is not self-contained on its own, since `SliceFile` references definitions owned by the `Ast`.
unsafe impl Send for CompilationData {}
unsafe impl Sync for CompilationData {}

#[derive(Debug)]
pub struct ConfigurationSet {
    pub slice_config: SliceConfig,
    pub compilation_data: CompilationData,
}

impl ConfigurationSet {
    /// Creates a new `ConfigurationSet` using the given root and built-in-slice paths.
    pub fn new(root_path: PathBuf, built_in_path: String) -> Self {
        let mut slice_config = SliceConfig::default();
        slice_config.set_workspace_root_path(root_path);
        slice_config.set_built_in_slice_path(Some(built_in_path));

        let compilation_data = CompilationData::default();
        Self { slice_config, compilation_data }
    }

    /// Parses a vector of `ConfigurationSet` from a JSON array, root path, and built-in path.
    pub fn parse_configuration_sets(
        config_array: &[serde_json::Value],
        root_path: &Path,
        built_in_path: &str,
    ) -> Vec<ConfigurationSet> {
        config_array
            .iter()
            .map(|value| ConfigurationSet::from_json(value, root_path, built_in_path))
            .collect::<Vec<_>>()
    }

    /// Constructs a `ConfigurationSet` from a JSON value.
    fn from_json(value: &serde_json::Value, root_path: &Path, built_in_path: &str) -> Self {
        // Parse the paths and `include_built_in_types` from the configuration set
        let paths = parse_paths(value);
        let include_built_in = parse_include_built_in(value);

        // Create the SliceConfig and CompilationState
        let mut slice_config = SliceConfig::default();
        slice_config.set_workspace_root_path(root_path.to_owned());
        slice_config.set_built_in_slice_path(include_built_in.then(|| built_in_path.to_owned()));
        slice_config.set_search_paths(paths);

        let compilation_data = CompilationData::default();
        Self { slice_config, compilation_data }
    }

    pub fn trigger_compilation(&mut self) -> Vec<Diagnostic> {
        // Perform the compilation.
        let slice_options = self.slice_config.as_slice_options();
        let compilation_state = slicec::compile_from_options(slice_options, |_| {}, |_| {});
        let CompilationState { ast, diagnostics, files } = compilation_state;

        // Process the diagnostics (filter out allowed lints, and update diagnostic levels as necessary).
        let updated_diagnostics = diagnostics.into_updated(&ast, &files, slice_options);

        // Store the data we got from compiling, then return the diagnostics so they can be published.
        self.compilation_data = CompilationData { ast, files };
        updated_diagnostics
    }
}

/// Parses paths from a JSON value.
fn parse_paths(value: &serde_json::Value) -> Vec<String> {
    value
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|dirs_array| {
            dirs_array
                .iter()
                .filter_map(|v| v.as_str())
                .map(sanitize_path)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

/// Determines whether to include built-in types from a JSON value.
fn parse_include_built_in(value: &serde_json::Value) -> bool {
    value
        .get("addWellKnownTypes")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}
