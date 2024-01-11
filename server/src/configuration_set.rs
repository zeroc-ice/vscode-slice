// Copyright (c) ZeroC, Inc.

use crate::slice_config;
use slice_config::SliceConfig;
use slicec::compilation_state::CompilationState;
use tower_lsp::lsp_types::Url;

pub struct ConfigurationSet {
    pub slice_config: SliceConfig,
    pub compilation_state: CompilationState,
}

impl ConfigurationSet {
    /// Creates a new `ConfigurationSet` using the given root URI and built-in path.
    pub fn new(root_uri: Url, built_in_path: String) -> Self {
        let mut slice_config = SliceConfig::default();
        slice_config.set_root_uri(root_uri);
        slice_config.set_built_in_path(built_in_path.to_owned());
        let compilation_state =
            slicec::compile_from_options(slice_config.as_slice_options(), |_| {}, |_| {});
        Self {
            slice_config,
            compilation_state,
        }
    }

    /// Parses a vector of `ConfigurationSet` from a JSON array, root URI, and built-in path.
    pub fn parse_configuration_sets(
        config_array: &[serde_json::Value],
        root_uri: &Url,
        built_in_path: &str,
    ) -> Vec<ConfigurationSet> {
        config_array
            .iter()
            .map(|value| ConfigurationSet::from_json(value, root_uri, built_in_path))
            .collect::<Vec<_>>()
    }

    /// Constructs a `ConfigurationSet` from a JSON value.
    fn from_json(value: &serde_json::Value, root_uri: &Url, built_in_path: &str) -> Self {
        // Parse the paths and `include_built_in_types` from the configuration set
        let paths = parse_paths(value);
        let include_built_in = parse_include_built_in(value);

        // Create the SliceConfig and CompilationState
        let mut slice_config = SliceConfig::default();
        slice_config.set_root_uri(root_uri.clone());
        slice_config.set_built_in_path(built_in_path.to_owned());
        slice_config.update_from_paths(paths);
        slice_config.update_include_built_in_path(include_built_in);

        let options = slice_config.as_slice_options();
        let compilation_state = slicec::compile_from_options(options, |_| {}, |_| {});
        Self {
            slice_config,
            compilation_state,
        }
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
                .map(String::from)
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
