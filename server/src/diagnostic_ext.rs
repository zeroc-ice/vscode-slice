// Copyright (c) ZeroC, Inc.

use crate::config::SliceConfig;
use crate::utils::convert_slice_url_to_uri;

use slicec::compilation_state::CompilationState;
use slicec::diagnostics::{Diagnostic, DiagnosticLevel, Note};
use std::collections::{HashMap, HashSet};
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{
    DiagnosticRelatedInformation, Location, NumberOrString, Position, Range, Url,
};
use tower_lsp::{lsp_types::*, Client};

/// Publishes diagnostics for all files in a given configuration set.
///
/// This function takes a client and a configuration set, generates updated diagnostics,
/// and then publishes these diagnostics to the LSP client.
pub async fn publish_diagnostics_for_all_files(
    client: &Client,
    configuration_set: (&SliceConfig, &mut CompilationState),
) {
    client
        .log_message(MessageType::INFO, "Publishing diagnostics...")
        .await;
    let compilation_state = configuration_set.1;

    // Extract and update diagnostics from the compilation state
    let diagnostics = std::mem::take(&mut compilation_state.diagnostics).into_updated(
        &compilation_state.ast,
        &compilation_state.files,
        configuration_set.0.as_slice_options(),
    );

    // Initialize a map to hold diagnostics grouped by file (URL)
    let mut map = compilation_state
        .files
        .keys()
        .filter_map(|uri| Some((convert_slice_url_to_uri(uri)?, vec![])))
        .collect::<HashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>>();

    // Process the diagnostics and populate the map
    process_diagnostics(diagnostics.iter().collect(), &mut map);

    // Publish the diagnostics for each file
    for (uri, lsp_diagnostics) in map {
        client.publish_diagnostics(uri, lsp_diagnostics, None).await;
    }
    client
        .log_message(MessageType::INFO, "Updated diagnostics for all files")
        .await;
}

/// Publishes diagnostics for all configuration sets.
///
/// This function iterates over all configuration sets and invokes
/// `publish_diagnostics_for_all_files` for each set.
pub async fn publish_diagnostics(
    client: &Client,
    configuration_sets: &Mutex<HashMap<SliceConfig, CompilationState>>,
) {
    let mut configuration_sets = configuration_sets.lock().await;

    for configuration_set in configuration_sets.iter_mut() {
        publish_diagnostics_for_all_files(client, configuration_set).await;
    }
}

/// Processes a list of diagnostics and updates the publish map with LSP-compatible diagnostics.
///
/// This function filters out any diagnostics that do not have a span or cannot be converted
/// to an LSP diagnostic. It then updates the given publish map with the processed diagnostics.
pub fn process_diagnostics(
    diagnostics: Vec<&slicec::diagnostics::Diagnostic>,
    publish_map: &mut HashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>,
) {
    diagnostics
        .into_iter()
        .filter_map(|diagnostic| {
            let span = diagnostic.span()?;
            let uri = convert_slice_url_to_uri(&span.file)?;
            try_into_lsp_diagnostic(diagnostic).map(|lsp_diagnostic| (uri, lsp_diagnostic))
        })
        .for_each(|(uri, lsp_diagnostic)| {
            publish_map.entry(uri).or_default().push(lsp_diagnostic);
        });
}

/// Clears the diagnostics for all tracked files in the configuration sets.
///
/// This function iterates over all configuration sets, collects all tracked file URIs,
/// and then publishes empty diagnostics to clear existing ones for each URI.
pub async fn clear_diagnostics(
    client: &Client,
    configuration_sets: &Mutex<HashMap<SliceConfig, CompilationState>>,
) {
    let configuration_sets = configuration_sets.lock().await;
    let mut all_tracked_files = HashSet::new();
    for configuration_set in configuration_sets.iter() {
        configuration_set
            .1
            .files
            .keys()
            .cloned()
            .filter_map(|uri| convert_slice_url_to_uri(&uri))
            .for_each(|uri| {
                all_tracked_files.insert(uri);
            });
    }

    // Clear diagnostics for each tracked file
    for uri in all_tracked_files {
        client.publish_diagnostics(uri, vec![], None).await;
    }
}

// A helper function that converts a slicec diagnostic into an lsp diagnostics
pub fn try_into_lsp_diagnostic(
    diagnostic: &Diagnostic,
) -> Option<tower_lsp::lsp_types::Diagnostic> {
    let severity = match diagnostic.level() {
        DiagnosticLevel::Error => Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
        DiagnosticLevel::Warning => Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
        DiagnosticLevel::Allowed => None,
    };

    // Map the spans to ranges, if span is none, return none
    let range = match diagnostic.span() {
        Some(span) => {
            let start = tower_lsp::lsp_types::Position::new(
                (span.start.row - 1) as u32,
                (span.start.col - 1) as u32,
            );
            let end = tower_lsp::lsp_types::Position::new(
                (span.end.row - 1) as u32,
                (span.end.col - 1) as u32,
            );
            Range::new(start, end)
        }
        None => return None,
    };

    let message = diagnostic.message();
    let related_information: Option<Vec<DiagnosticRelatedInformation>> = Some(
        diagnostic
            .notes()
            .iter()
            .filter_map(try_into_lsp_diagnostic_related_information)
            .collect(),
    );

    Some(tower_lsp::lsp_types::Diagnostic {
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
    let span = note.span.as_ref()?;
    let file_path = Url::from_file_path(span.file.clone()).ok()?;
    let start_position = Position::new((span.start.row - 1) as u32, (span.start.col - 1) as u32);
    let end_position = Position::new((span.end.row - 1) as u32, (span.end.col - 1) as u32);

    Some(DiagnosticRelatedInformation {
        location: Location {
            uri: file_path,
            range: Range {
                start: start_position,
                end: end_position,
            },
        },
        message: note.message.clone(),
    })
}
