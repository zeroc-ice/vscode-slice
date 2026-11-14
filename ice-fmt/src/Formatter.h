// Copyright (c) ZeroC, Inc.

#ifndef ICE_FMT_FORMATTER_H
#define ICE_FMT_FORMATTER_H

#include "SyntaxError.h"

#include <string>
#include <string_view>

namespace IceFmt
{
    /// Formats the text of a Slice file in the style that the files in tests/formatted show. Formatting never adds or
    /// reorders a token, and the only token it removes is the optional semicolon after a closing brace.
    /// @param text The text of the file.
    /// @return The formatted text, with the line endings of the first line of @p text and its byte order mark when it
    /// has one, or @p text as it is when it holds the comment `// ice-fmt: ignore-file`.
    /// @throws SyntaxError Thrown when @p text doesn't follow the Slice grammar, or holds a comment or preprocessor
    /// directive that formatting can't keep as the Slice compiler reads it.
    /// @throws std::logic_error Thrown when the formatted text would not keep the tokens and comments of @p text, or
    /// the definition a doc comment documents, which is a bug in the formatter.
    [[nodiscard]] std::string formatSlice(std::string_view text);
}

#endif
