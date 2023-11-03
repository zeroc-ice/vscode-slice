// Copyright (c) ZeroC, Inc.

use slicec::diagnostics::{Diagnostic, DiagnosticLevel, Note};
use tower_lsp::lsp_types::{
    CodeDescription, DiagnosticRelatedInformation, Location, NumberOrString, Position, Range, Url,
};

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
            .filter_map(|n| try_into_lsp_diagnostic_related_information(n))
            .collect(),
    );

    Some(tower_lsp::lsp_types::Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diagnostic.code().to_owned())),
        code_description: Some(CodeDescription { href:
                // Create a URL object to https://docs.icerpc.dev
                Url::parse("https://docs.icerpc.dev").unwrap(),
             }),
        source: Some("slicec".to_owned()),
        message,
        related_information,
        tags: None,
        data: None,
    })
}

fn try_into_lsp_diagnostic_related_information(
    note: &Note,
) -> Option<tower_lsp::lsp_types::DiagnosticRelatedInformation> {
    if let Some(span) = note.span.as_ref() {
        let file_path = Url::from_file_path(span.file.clone()).unwrap();
        let start_position =
            Position::new((span.start.row - 1) as u32, (span.start.col - 1) as u32);
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
    } else {
        None
    }
}
