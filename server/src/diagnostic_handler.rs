// Copyright (c) ZeroC, Inc.

use crate::slice_project::SliceProject;
use crate::utils::{convert_slice_path_to_uri, span_to_range};
use crate::{notifications, show_popup};

use slicec::diagnostics::{Diagnostic, DiagnosticLevel, Note};
use std::collections::{HashMap, HashSet};
use tower_lsp::lsp_types::{DiagnosticRelatedInformation, Location, NumberOrString, Url};
use tower_lsp::Client;

/// Publishes diagnostics for all files in the provided project.
///
/// This function takes a client and a project, generates updated diagnostics,
/// and then publishes these diagnostics to the LSP client.
pub async fn publish_diagnostics_for_project(
    client_handle: &Client,
    diagnostics: Vec<Diagnostic>,
    project: &mut SliceProject,
) {
    // Initialize a map to hold diagnostics grouped by file (URL)
    let mut map = project
        .compilation_data
        .files
        .keys()
        .filter_map(|uri| Some((convert_slice_path_to_uri(uri)?, vec![])))
        .collect::<HashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>>();

    // Process the diagnostics and populate the map.
    let spanless_diagnostics = process_diagnostics(diagnostics, &mut map);
    for diagnostic in spanless_diagnostics {
        show_popup(
            client_handle,
            diagnostic.message(),
            notifications::MessageType::Error,
        )
        .await;
    }

    // Publish the diagnostics for each file
    for (uri, lsp_diagnostics) in map {
        client_handle.publish_diagnostics(uri, lsp_diagnostics, None).await;
    }
}

/// Processes a list of diagnostics and updates the publish map with LSP-compatible diagnostics.
///
/// This function filters out any diagnostics that do not have a span or cannot be converted
/// to an LSP diagnostic. It then updates the given publish map with the processed diagnostics.
/// Any diagnostics that do not have a span are returned for further processing.
pub fn process_diagnostics(
    diagnostics: Vec<slicec::diagnostics::Diagnostic>,
    publish_map: &mut HashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>,
) -> Vec<slicec::diagnostics::Diagnostic> {
    let mut spanless_diagnostics = Vec::new();
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| {
            let span = diagnostic.span().cloned();
            match try_into_lsp_diagnostic(diagnostic) {
                Ok(lsp_diagnostic) => {
                    // The empty span case is handled by the `try_into_lsp_diagnostic` function.
                    let file = span
                        .expect("If the span was empty, try_into_lsp_diagnostic should have hit the error case")
                        .file;
                    let uri = convert_slice_path_to_uri(file)?;
                    Some((uri, lsp_diagnostic))
                }
                Err(diagnostic) => {
                    spanless_diagnostics.push(diagnostic);
                    None
                }
            }
        })
        .for_each(|(uri, lsp_diagnostic)| {
            publish_map.entry(uri).or_default().push(lsp_diagnostic);
        });
    spanless_diagnostics
}

/// Clears the diagnostics for all tracked files in the provided projects.
///
/// This function iterates over the projects, collects all tracked file URIs,
/// and then publishes empty diagnostics to clear existing ones for each URI.
pub async fn clear_diagnostics(client_handle: &Client, projects: &[SliceProject]) {
    let mut all_tracked_files = HashSet::new();
    for project in projects.iter() {
        project
            .compilation_data
            .files
            .keys()
            .filter_map(convert_slice_path_to_uri)
            .for_each(|uri| {
                all_tracked_files.insert(uri);
            });
    }

    // Clear diagnostics for each tracked file
    for uri in all_tracked_files {
        client_handle.publish_diagnostics(uri, vec![], None).await;
    }
}

// A helper function that converts a slicec diagnostic into an lsp diagnostics
#[allow(clippy::result_large_err)]
pub fn try_into_lsp_diagnostic(
    diagnostic: Diagnostic,
) -> Result<tower_lsp::lsp_types::Diagnostic, slicec::diagnostics::Diagnostic> {
    let severity = match diagnostic.level() {
        DiagnosticLevel::Error => Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        DiagnosticLevel::Warning => Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
        DiagnosticLevel::Allowed => None,
    };

    // Map the spans to ranges, if span is none, return the slicec diagnostic
    let range = match diagnostic.span() {
        Some(span) => span_to_range(span.clone()),
        None => return Err(diagnostic),
    };

    let message = diagnostic.message();
    let related_information: Option<Vec<DiagnosticRelatedInformation>> = Some(
        diagnostic
            .notes()
            .iter()
            .filter_map(try_into_lsp_diagnostic_related_information)
            .collect(),
    );

    Ok(tower_lsp::lsp_types::Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diagnostic.code().to_owned())),
        code_description: None,
        source: Some("slicec".to_owned()),
        message,
        related_information,
        tags: None,
        data: None,
    })
}

// A helper function that converts a slicec note into an lsp diagnostic related information
fn try_into_lsp_diagnostic_related_information(
    note: &Note,
) -> Option<tower_lsp::lsp_types::DiagnosticRelatedInformation> {
    let span = note.span.clone()?;
    let uri = convert_slice_path_to_uri(&span.file)?;
    let range = span_to_range(span);

    Some(DiagnosticRelatedInformation {
        location: Location { uri, range },
        message: note.message.clone(),
    })
}
