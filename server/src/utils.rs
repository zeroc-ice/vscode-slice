// Copyright (c) ZeroC, Inc.

use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;

// This helper function converts a Url from tower_lsp into a path that can be used to
// retrieve a file from the compilation state from slicec.
pub fn url_to_sanitized_file_path(url: &Url) -> Option<PathBuf> {
    let path = url.to_file_path().ok()?;
    let path_string = path.to_str()?;
    Some(PathBuf::from(sanitize_path(path_string)))
}

pub fn convert_slice_path_to_uri(path: impl AsRef<Path>) -> Option<Url> {
    Url::from_file_path(path).ok()
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
            let colon_index = sanitized_path.find(':').expect("no colon in disk prefix");
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
