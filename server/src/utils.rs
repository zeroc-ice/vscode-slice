// Copyright (c) ZeroC, Inc.
use crate::config::SliceConfig;
use serde_json::Value;
use slicec::compilation_state::CompilationState;
use tower_lsp::lsp_types::Url;

// A helper trait that allows us to find a file in an iterator of (&SliceConfig, &CompilationState)
pub trait FindFile<'a> {
    type Item;
    fn find_file(self, file_name: &str) -> Option<Self::Item>;
}

// Implement the trait for all types that implement Iterator<Item = (&'a SliceConfig, &'a CompilationState)>
impl<'a, I> FindFile<'a> for I
where
    I: Iterator<Item = (&'a SliceConfig, &'a CompilationState)>,
{
    type Item = (&'a SliceConfig, &'a CompilationState);

    fn find_file(mut self, file_name: &str) -> Option<Self::Item> {
        self.find(|(_config, state)| state.files.keys().any(|key| key == file_name))
    }
}

// This helper function converts a Url from tower_lsp into a string that can be used to
// retrieve a file from the compilation state from slicec.
pub fn convert_uri_to_slice_formated_url(uri: Url) -> Option<String> {
    Some(
        uri.to_file_path()
            .ok()?
            .to_path_buf()
            .as_path()
            .to_str()?
            .to_owned(),
    )
}

pub fn convert_slice_url_to_uri(url: &str) -> Option<Url> {
    Url::from_file_path(url).ok()
}

pub fn url_to_file_path(url: &Url) -> Option<String> {
    Some(url.to_file_path().ok()?.to_str()?.to_owned())
}

pub fn new_configuration_set(
    root_uri: Url,
    built_in_path: String,
) -> (SliceConfig, CompilationState) {
    let mut configuration = SliceConfig::default();
    configuration.set_root_uri(root_uri);
    configuration.set_built_in_reference(built_in_path.to_owned());
    let compilation_state =
        slicec::compile_from_options(configuration.as_slice_options(), |_| {}, |_| {});
    (configuration, compilation_state)
}

pub fn parse_configuration_sets(
    config_array: &[Value],
    root_uri: &Url,
    built_in_path: &str,
) -> Vec<(SliceConfig, CompilationState)> {
    config_array
        .iter()
        .map(|config_obj| {
            let directories = config_obj
                .get("referenceDirectories")
                .and_then(|v| v.as_array())
                .map(|dirs_array| {
                    dirs_array
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // If the includeBuiltInTypes is not specified, default to true
            let include_built_in = config_obj
                .get("includeBuiltInTypes")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            (directories, include_built_in)
        })
        .map(|config| {
            let mut slice_config = SliceConfig::default();

            slice_config.set_root_uri(root_uri.clone());
            slice_config.set_built_in_reference(built_in_path.to_owned());
            slice_config.update_from_references(config.0);
            slice_config.update_include_built_in_reference(config.1);

            let options = slice_config.as_slice_options();
            let compilation_state = slicec::compile_from_options(options, |_| {}, |_| {});

            (slice_config, compilation_state)
        })
        .collect::<Vec<_>>()
}
