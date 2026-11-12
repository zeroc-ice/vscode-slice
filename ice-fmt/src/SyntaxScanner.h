// Copyright (c) ZeroC, Inc.

#ifndef ICE_FMT_SYNTAX_SCANNER_H
#define ICE_FMT_SYNTAX_SCANNER_H

#include <string_view>
#include <vector>

namespace IceFmt
{
    enum class TokenKind
    {
        /// A name without a scope, such as `Identity` or the escaped keyword `\module`.
        Identifier,
        /// A name with a scope, such as `::Ice::Identity`.
        ScopedIdentifier,
        IntegerLiteral,
        FloatingPointLiteral,
        StringLiteral,

        // The keywords.
        Module,
        Class,
        Interface,
        Exception,
        Struct,
        Sequence,
        Dictionary,
        Enum,
        Out,
        Extends,
        Throws,
        Void,
        Bool,
        Byte,
        Short,
        Int,
        Long,
        Float,
        Double,
        String,
        Object,
        Value,
        Const,
        False,
        True,
        Idempotent,
        Optional,

        // The punctuation marks.
        LeftBrace,
        RightBrace,
        LeftParenthesis,
        RightParenthesis,
        LeftAngle,
        RightAngle,
        LeftBracket,
        RightBracket,
        /// `[[`, which opens file metadata.
        DoubleLeftBracket,
        /// `]]`, which closes file metadata.
        DoubleRightBracket,
        Comma,
        Semicolon,
        Equals,
        Asterisk,
        Colon,

        /// A `//` comment, with its continuation lines.
        LineComment,
        BlockComment,
        /// A preprocessor directive, with its continuation lines.
        Directive,
        /// A character that no token starts with.
        Invalid,
        EndOfFile,
    };

    /// A token, comment, or preprocessor directive in the text of a Slice file.
    struct Lexeme
    {
        TokenKind kind;

        /// A view into the scanned text.
        std::string_view text;

        /// The one-based line of the first character.
        int line;

        /// The zero-based column of the first character, in bytes.
        int column;
    };

    /// Returns whether the Slice compiler reads a lexeme as a doc comment: `///`, or a block comment `/**` where
    /// neither `/` nor `*` follows.
    [[nodiscard]] bool isDocComment(const Lexeme& lexeme);

    /// Returns how a punctuation mark is written, such as `;` for TokenKind::Semicolon.
    /// @param kind The kind of a punctuation mark.
    [[nodiscard]] std::string_view spelling(TokenKind kind);

    /// Splits the text of a Slice file into lexemes. Unlike the Slice compiler's scanner, it reads text that wasn't
    /// preprocessed and keeps the comments and preprocessor directives.
    /// @param text The text of the file. The lexemes are views into it.
    /// @return The lexemes in order, ending with an EndOfFile lexeme.
    [[nodiscard]] std::vector<Lexeme> scan(std::string_view text);
}

#endif
