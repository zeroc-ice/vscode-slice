// Copyright (c) ZeroC, Inc.

#ifndef SLICE_FORMAT_FORMATTER_H
#define SLICE_FORMAT_FORMATTER_H

#include "SyntaxError.h"

#include <string>
#include <string_view>

namespace SliceFormat
{
    /// Formats the text of a Slice file: four spaces for each level of indentation, a single space or none between
    /// tokens, each brace on its own line, and lines of at most 120 columns. A list of parameters, names, or metadata
    /// directives is on one line when it fits, and has one item per line otherwise. An operation that doesn't fit on
    /// one line gives the line above to its metadata, then a line to its throws clause, then a line to each
    /// parameter, and stops at the first step that makes it fit.
    ///
    /// The formatter keeps the author's choice where the Slice files of Ice vary: a blank line between definitions,
    /// a line break after metadata, and a definition written on a single line. An empty body written as {} follows
    /// its definition on the same line, and a blank line separates two definitions that each have a body. The
    /// formatter never adds or reorders a token or a comment, and the only token it removes is the optional
    /// semicolon after a closing brace.
    /// @param text The text of the file.
    /// @return The formatted text, with the line endings of @p text and its byte order mark when it has one.
    /// @throws SyntaxError Thrown when @p text doesn't follow the Slice grammar.
    [[nodiscard]] std::string formatSlice(std::string_view text);
}

#endif
