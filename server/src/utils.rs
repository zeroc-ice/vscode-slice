// Copyright (c) ZeroC, Inc.
use crate::configuration_set::ConfigurationSet;
use slicec::compilation_state::CompilationState;
use tower_lsp::lsp_types::Url;

// A helper trait that allows us to find a file in an iterator of (&SliceConfig, &CompilationState)
pub trait FindFile<'a> {
    fn find_file(self, file_name: &str) -> Option<&'a CompilationState>;
}

impl<'a, I> FindFile<'a> for I
where
    I: Iterator<Item = &'a ConfigurationSet>,
{
    fn find_file(mut self, file_name: &str) -> Option<&'a CompilationState> {
        self.find(|set| {
            set.compilation_state
                .files
                .keys()
                .any(|key| key == file_name)
        })
        .map(|set| &set.compilation_state)
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
