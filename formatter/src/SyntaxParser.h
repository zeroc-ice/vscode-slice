// Copyright (c) ZeroC, Inc.

#ifndef SLICE_FORMAT_SYNTAX_PARSER_H
#define SLICE_FORMAT_SYNTAX_PARSER_H

#include "SyntaxError.h"
#include "SyntaxScanner.h"

#include <string_view>
#include <utility>
#include <vector>

namespace SliceFormat
{
    /// A comment or preprocessor directive, attached to the token it sits next to.
    struct Trivia
    {
        /// The text, without the blanks that indent it.
        std::string_view text;

        /// The zero-based column of the first character, in bytes.
        int column;

        /// Whether this trivia has its lines to itself. Otherwise it shares a line with the token it is attached to.
        bool ownLine;

        /// Whether a blank line separates this trivia from what precedes it.
        bool blankLineBefore;
    };

    struct Token
    {
        TokenKind kind;

        /// A view into the parsed text.
        std::string_view text;

        /// The one-based line of the token.
        int line;

        /// Whether this token is the first one on its line.
        bool startsLine;

        /// Whether a blank line separates this token from what precedes it, its own leading trivia included.
        bool blankLineBefore;

        /// The trivia between the previous token's trailing trivia and this token.
        std::vector<Trivia> leading;

        /// The comments that follow this token on its line, and end that line.
        std::vector<Trivia> trailing;
    };

    enum class NodeKind
    {
        Token,

        /// A whole file: its definitions and file metadata, then the EndOfFile token.
        File,

        /// A definition, member, enumerator, or parameter, from its metadata to its last token.
        Declaration,

        /// Metadata or file metadata, from the opening bracket to the closing one.
        Metadata,

        /// The parameters of an operation, from '(' to ')'.
        Parameters,

        /// An 'extends' or 'throws' clause.
        Clause,

        /// The members of a definition, from '{' to '}'.
        Body,
    };

    /// A node of the syntax tree of a Slice file. The tree holds every token of the file, in order.
    struct Node
    {
        /// Creates a node without children.
        explicit Node(NodeKind nodeKind) : kind{nodeKind} {}

        /// Creates a Token node.
        explicit Node(Token nodeToken) : kind{NodeKind::Token}, token{std::move(nodeToken)} {}

        NodeKind kind;

        /// The token of a Token node.
        Token token{};

        /// The children of any other node.
        std::vector<Node> children;
    };

    /// Parses the text of a Slice file. The parser checks the syntax only: it accepts a file that the Slice compiler
    /// rejects for what its definitions mean.
    /// @param text The text of the file. The tokens of the tree are views into it.
    /// @return The File node.
    /// @throws SyntaxError Thrown when @p text doesn't follow the Slice grammar.
    [[nodiscard]] Node parse(std::string_view text);
}

#endif
