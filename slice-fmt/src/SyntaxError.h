// Copyright (c) ZeroC, Inc.

#ifndef SLICE_FORMAT_SYNTAX_ERROR_H
#define SLICE_FORMAT_SYNTAX_ERROR_H

#include <stdexcept>
#include <string>

namespace SliceFormat
{
    /// The exception that is thrown when the text of a Slice file doesn't follow the Slice grammar.
    class SyntaxError final : public std::runtime_error
    {
    public:
        /// @param errorLine The one-based line where the text stops following the grammar.
        /// @param message What the parser expected there, and what it found.
        SyntaxError(int errorLine, const std::string& message) : std::runtime_error{message}, line{errorLine} {}

        /// The one-based line where the text stops following the grammar.
        int line;
    };
}

#endif
