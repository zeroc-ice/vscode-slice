// Copyright (c) ZeroC, Inc.

#ifndef ICE_FMT_SYNTAX_ERROR_H
#define ICE_FMT_SYNTAX_ERROR_H

#include <stdexcept>
#include <string>

namespace IceFmt
{
    /// The exception that is thrown when the text of a Slice file can't be formatted: it doesn't follow the Slice
    /// grammar, or it holds a comment or directive that formatting can't keep as the Slice compiler reads it.
    class SyntaxError final : public std::runtime_error
    {
    public:
        /// @param errorLine The one-based line of the error.
        /// @param message What the parser expected there and what it found, or what it refuses.
        SyntaxError(int errorLine, const std::string& message) : std::runtime_error{message}, line{errorLine} {}

        /// The one-based line of the error.
        int line;
    };
}

#endif
