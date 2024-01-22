// Copyright (c) ZeroC, Inc.

use crate::configuration_set::{CompilationData, ConfigurationSet};
use std::path::Path;
use tower_lsp::lsp_types::Url;

// A helper trait that allows us to find a file in an iterator of ConfigurationSet.
pub trait FindFile<'a> {
    fn find_file(self, file_name: &str) -> Option<&'a CompilationData>;
}

impl<'a, I> FindFile<'a> for I
where
    I: Iterator<Item = &'a ConfigurationSet>,
{
    fn find_file(mut self, file_name: &str) -> Option<&'a CompilationData> {
        self.find(|set| {
            set.compilation_data.files.keys().any(|f| {
                let key_path = Path::new(f);
                let file_path = Path::new(file_name);
                key_path == file_path || file_path.starts_with(key_path)
            })
        })
        .map(|set| &set.compilation_data)
    }
}

pub fn convert_slice_url_to_uri(url: &str) -> Option<Url> {
    Url::from_file_path(url).ok()
}

// This helper function converts a Url from tower_lsp into a string that can be used to
// retrieve a file from the compilation state from slicec.
pub fn url_to_file_path(url: &Url) -> Option<String> {
    Some(url.to_file_path().ok()?.to_str()?.to_owned())
}
