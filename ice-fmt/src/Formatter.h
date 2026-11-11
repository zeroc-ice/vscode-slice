// Copyright (c) ZeroC, Inc.

#ifndef ICE_FMT_FORMATTER_H
#define ICE_FMT_FORMATTER_H

#include "SyntaxError.h"

#include <string>
#include <string_view>

namespace IceFmt
{
    /// Formats the text of a Slice file in the style that README.md describes. Formatting never adds or reorders a
    /// token, and the only token it removes is the optional semicolon after a closing brace.
    /// @param text The text of the file.
    /// @return The formatted text, with the line endings of the first line of @p text and its byte order mark when it
    /// has one.
    /// @throws SyntaxError Thrown when @p text doesn't follow the Slice grammar, or holds a comment or directive that
    /// formatting can't keep as the Slice compiler reads it.
    [[nodiscard]] std::string formatSlice(std::string_view text);
}

#endif
