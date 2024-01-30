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

pub fn url_to_sanitized_file_path(url: &Url) -> Option<String> {
    let path = url.to_file_path().ok()?;
    let path_string = path.to_str()?;
    Some(sanitize_path(path_string))
}

pub fn convert_slice_url_to_uri(url: &str) -> Option<Url> {
    Url::from_file_path(url).ok()
}

#[cfg(target_os = "windows")]
pub fn sanitize_path(s: &str) -> String {
    use std::path::{Component, Prefix};

    // Replace any forward-slashes with back-slashes.
    let mut sanitized_path = s.replace('/', "\\");

    // Check if the path begins with a disk prefix (windows), and if so, make sure it's capitalized.
    if let Some(Component::Prefix(prefix)) = Path::new(&sanitized_path).components().next() {
        if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) {
            // disk prefixes are always of the form 'C:' or '\\?\C:'
            let colon_index = sanitized_path.find(':').expect("no colon found in disk prefix");
            let disk_prefix = sanitized_path.split_at_mut(colon_index).0;

             // Windows disk prefixes only use ascii characters.
            assert!(disk_prefix.is_ascii());
            disk_prefix.make_ascii_uppercase()
        }
    }

    sanitized_path
}

#[cfg(not(target_os = "windows"))]
pub fn sanitize_path(s: &str) -> String {
    s.to_owned()
}
