// Copyright (c) ZeroC, Inc.

use crate::slice_config::{compute_slice_options, ServerConfig, SliceConfig};
use crate::utils::sanitize_path;
use std::collections::HashMap;
use std::path::PathBuf;
use slicec::slice_options::SliceOptions;
use slicec::{ast::Ast, diagnostics::Diagnostic, slice_file::SliceFile};
use slicec::compilation_state::CompilationState;

#[derive(Debug, Default)]
pub struct CompilationData {
    pub ast: Ast,
    pub files: HashMap<PathBuf, SliceFile>,
}

// Necessary for using `CompilationData` within async functions.
//
// # Safety
//
// These implementations are safe because `CompilationData` is entirely self-contained and hence can go between threads.
// Note that `files` is not self-contained on its own, since `SliceFile` references definitions owned by the `Ast`.
unsafe impl Send for CompilationData {}
unsafe impl Sync for CompilationData {}

#[derive(Debug, Default)]
pub struct ConfigurationSet {
    pub slice_config: SliceConfig,
    pub compilation_data: CompilationData,

    cached_slice_options: Option<SliceOptions>,
}

impl ConfigurationSet {
    /// Parses a vector of `ConfigurationSet` from a JSON array.
    pub fn parse_configuration_sets(config_array: &[serde_json::Value]) -> Vec<Self> {
        config_array
            .iter()
            .map(ConfigurationSet::from_json)
            .collect::<Vec<_>>()
    }

    /// Constructs a `ConfigurationSet` from a JSON value.
    fn from_json(value: &serde_json::Value) -> Self {
        let slice_config = SliceConfig {
            slice_search_paths: parse_paths(value),
            include_built_in_slice_files: parse_include_built_in(value),
        };
        Self { slice_config, ..Self::default() }
    }

    pub fn trigger_compilation(&mut self, server_config: &ServerConfig) -> Vec<Diagnostic> {
        // Re-compute the `slice_options` we're going to pass into the compiler, if necessary.
        let slice_options = self.cached_slice_options.get_or_insert_with(|| {
            compute_slice_options(server_config, &self.slice_config)
        });

        // Perform the compilation.
        let compilation_state = slicec::compile_from_options(slice_options, |_| {}, |_| {});
        let CompilationState { ast, diagnostics, files } = compilation_state;

        // Process the diagnostics (filter out allowed lints, and update diagnostic levels as necessary).
        let updated_diagnostics = diagnostics.into_updated(&ast, &files, slice_options);

        // Convert the stringified paths returned by `slicec` to actual PathBuf objects.
        let files = files.into_iter().map(|f| (PathBuf::from(&f.relative_path), f)).collect();

        // Store the data we got from compiling, then return the diagnostics so they can be published.
        self.compilation_data = CompilationData { ast, files };
        updated_diagnostics
    }
}

/// Parses paths from a JSON value.
fn parse_paths(value: &serde_json::Value) -> Vec<PathBuf> {
    value
        .get("paths")
        .and_then(|v| v.as_array())
        .map(|dirs_array| {
            dirs_array
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| PathBuf::from(sanitize_path(s)))
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
