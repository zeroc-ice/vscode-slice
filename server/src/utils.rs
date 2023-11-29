// Copyright (c) ZeroC, Inc.

use tower_lsp::lsp_types::Url;

// This helper function converts a Url from tower_lsp into a string that can be used to
// retrieve a file from the compilation state from slicec.
pub fn convert_uri_to_slice_formated_url(uri: Url) -> Option<String> {
    Some(uri.to_file_path().ok()?.as_path().to_str()?.to_owned())
}

pub fn convert_slice_url_to_uri(url: &str) -> Option<Url> {
    Url::from_file_path(url).ok()
}
