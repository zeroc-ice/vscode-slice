// Copyright (c) ZeroC, Inc.

#include "Formatter.h"
#include "Layout.h"
#include "SyntaxParser.h"
#include "SyntaxScanner.h"

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

using namespace std;
using namespace IceFmt;

namespace
{
    // The tokens that no space follows, and the tokens that no space precedes.
    constexpr array noSpaceAfter{
        TokenKind::LeftParenthesis,
        TokenKind::LeftAngle,
        TokenKind::LeftBracket,
        TokenKind::DoubleLeftBracket};
    constexpr array noSpaceBefore{
        TokenKind::RightParenthesis,
        TokenKind::RightAngle,
        TokenKind::RightBracket,
        TokenKind::DoubleRightBracket,
        TokenKind::Comma,
        TokenKind::Semicolon,
        TokenKind::LeftParenthesis,
        TokenKind::LeftAngle,
        TokenKind::Asterisk};

    Layout print(const Node& node);

    bool contains(const auto& kinds, TokenKind kind) { return ranges::find(kinds, kind) != kinds.end(); }

    const Token& firstToken(const Node& node)
    {
        return node.kind == NodeKind::Token ? node.token : firstToken(node.children.front());
    }

    const Token& lastToken(const Node& node)
    {
        return node.kind == NodeKind::Token ? node.token : lastToken(node.children.back());
    }

    bool isToken(const Node& node, TokenKind kind) { return node.kind == NodeKind::Token && node.token.kind == kind; }

    // Returns whether the author started a line with this node.
    bool startsLine(const Node& node) { return firstToken(node).startsLine; }

    // Returns whether the author left a blank line before this node and the comments that lead it.
    bool blankLineBefore(const Node& node)
    {
        const Token& token = firstToken(node);
        return token.leading.empty() ? token.blankLineBefore : token.leading.front().blankLineBefore;
    }

    // Returns whether a node is a container: a module, class, interface, exception, struct, or enum. Unlike the Slice
    // compiler, it doesn't count an operation as the container of its parameters.
    bool isContainer(const Node& node)
    {
        return ranges::find(node.children, NodeKind::Body, &Node::kind) != node.children.end();
    }

    // Returns the number of metadata lists that lead a node.
    size_t leadingMetadata(const Node& node)
    {
        size_t count = 0;
        while (node.children[count].kind == NodeKind::Metadata)
        {
            ++count;
        }
        return count;
    }

    // Returns the index of the keyword of a definition, after its metadata and the 'local' of Ice 3.7.
    size_t keywordIndex(const Node& definition)
    {
        size_t index = leadingMetadata(definition);
        return definition.children[index].token.text == "local" ? index + 1 : index;
    }

    // Returns whether a node defines or forward declares a module, class, interface, exception, struct, or enum.
    bool declaresContainer(const Node& node)
    {
        constexpr array keywords{
            TokenKind::Module,
            TokenKind::Class,
            TokenKind::Interface,
            TokenKind::Exception,
            TokenKind::Struct,
            TokenKind::Enum};
        return contains(keywords, node.children[keywordIndex(node)].token.kind);
    }

    // Returns whether a node is the forward declaration of a definition, as 'class Node;' is of 'class Node { ... }'.
    bool forwardDeclares(const Node& node, const Node& definition)
    {
        if (isContainer(node) || !isContainer(definition))
        {
            return false;
        }

        // The name follows the keyword. Only a forward declaration starts with the keyword of a container without
        // being one.
        size_t first = keywordIndex(node);
        size_t definitionFirst = keywordIndex(definition);
        return node.children[first].token.kind == definition.children[definitionFirst].token.kind &&
               node.children[first + 1].token.text == definition.children[definitionFirst + 1].token.text;
    }

    // Returns whether a blank line separates a node from the one before it: the author left one, or both are contained,
    // at least one of them is a container, and nothing separates them yet. A forward declaration stays against the
    // definition it declares. A preprocessor directive between the two leaves the choice to the author, since the
    // directive can belong to either one.
    bool blankLineBetween(const Node& previous, const Node& node)
    {
        if (blankLineBefore(node))
        {
            return true;
        }
        if (previous.kind != NodeKind::Contained || node.kind != NodeKind::Contained)
        {
            return false;
        }
        const Token& token = firstToken(node);
        bool separated = token.blankLineBefore || ranges::any_of(token.leading, &Trivia::blankLineBefore);
        auto isDirective = [](const Trivia& trivia) { return trivia.text.starts_with('#'); };
        return (isContainer(previous) || isContainer(node)) && !separated && !forwardDeclares(previous, node) &&
               ranges::none_of(token.leading, isDirective);
    }

    bool needsSpace(const Node& left, const Node& right)
    {
        // A body and a clause start with a line break of their own.
        if (right.kind == NodeKind::Body || right.kind == NodeKind::Clause)
        {
            return false;
        }

        if (contains(noSpaceAfter, lastToken(left).kind))
        {
            return false;
        }

        // A comment that sits before the token on its line is spaced like a name.
        const Token& token = firstToken(right);
        bool commentFirst = !token.leading.empty() && !token.leading.front().ownLine;
        return commentFirst || !contains(noSpaceBefore, token.kind);
    }

    // Returns a line of a comment or preprocessor directive without the blanks and the carriage return that end it.
    string_view withoutTrailingBlanks(string_view line) { return line.substr(0, line.find_last_not_of(" \t\r") + 1); }

    // Splits a comment or preprocessor directive into its lines. A line after the first one keeps the indentation it
    // has relative to the first one.
    vector<string> splitLines(const Trivia& trivia)
    {
        auto column = static_cast<size_t>(trivia.column);
        vector<string> lines;
        string_view rest = trivia.text;
        while (true)
        {
            size_t end = rest.find('\n');
            string_view current = withoutTrailingBlanks(rest.substr(0, end));
            size_t indentation = current.find_first_not_of(" \t");
            if (lines.empty() || current.empty())
            {
                lines.emplace_back(current);
            }
            else
            {
                string indented(indentation > column ? indentation - column : 0, ' ');
                indented += current.substr(indentation);
                lines.push_back(std::move(indented));
            }

            if (end == string_view::npos)
            {
                return lines;
            }
            rest.remove_prefix(end + 1);
        }
    }

    // The comments and preprocessor directives before a token, without the blank line that precedes the first one or
    // the space that follows the last one.
    Layout leadingTrivia(const vector<Trivia>& leading)
    {
        vector<Layout> parts;
        for (const Trivia& trivia : leading)
        {
            if (!parts.empty() && trivia.blankLineBefore)
            {
                // The first line break ends the previous comment.
                parts.push_back(hardline());
                parts.push_back(hardline());
            }

            if (trivia.ownLine)
            {
                vector<string> lines = splitLines(trivia);
                parts.push_back(endOfLine());
                parts.push_back(text(lines.front()));
                for (size_t i = 1; i < lines.size(); ++i)
                {
                    parts.push_back(hardline());
                    parts.push_back(text(lines[i]));
                }
                parts.push_back(endOfLine());
            }
            else
            {
                if (!parts.empty())
                {
                    parts.push_back(space());
                }
                parts.push_back(text(string{trivia.text}));
            }
        }
        return concat(std::move(parts));
    }

    // The comments that follow a token on its line.
    Layout trailingTrivia(const Token& token)
    {
        vector<Layout> parts;
        parts.reserve(token.trailing.size());
        for (const Trivia& trivia : token.trailing)
        {
            parts.push_back(suffix(string{withoutTrailingBlanks(trivia.text)}));
        }
        return concat(std::move(parts));
    }

    // The text of a token and the comments that follow it on its line.
    Layout textAndTrailingTrivia(const Token& token) { return concat(text(string{token.text}), trailingTrivia(token)); }

    // What precedes the text of a token: the comments and preprocessor directives before it, then the blank line or the
    // space that separates the last one from the token.
    Layout beforeText(const Token& token)
    {
        vector<Layout> parts;
        parts.push_back(leadingTrivia(token.leading));
        if (!token.leading.empty())
        {
            if (token.blankLineBefore)
            {
                parts.push_back(hardline());
                parts.push_back(hardline());
            }
            if (!token.leading.back().ownLine && !contains(noSpaceBefore, token.kind))
            {
                parts.push_back(space());
            }
        }
        return concat(std::move(parts));
    }

    Layout print(const Token& token) { return concat(beforeText(token), textAndTrailingTrivia(token)); }

    // Groups a list that starts with the token 'open'. The comments before that token would break the group, so the
    // caller lays them out before it.
    Layout listGroup(const Token& open, Layout items)
    {
        return group(concat(textAndTrailingTrivia(open), indent(std::move(items))));
    }

    // What separates a child from the child before it. 'afterComma' is what follows a comma.
    Layout separator(const Node& previous, const Node& child, const Layout& afterComma)
    {
        if (isToken(previous, TokenKind::Comma))
        {
            return afterComma;
        }
        if (previous.kind == NodeKind::Metadata)
        {
            // Metadata stays on the line of the type or name it applies to. Two metadata lists keep the line break the
            // author put between them.
            bool ownLine = child.kind == NodeKind::Metadata && startsLine(child);
            return ownLine ? hardline() : space();
        }
        return needsSpace(previous, child) ? space() : concat();
    }

    // Appends the children of a node from index 'first' to the one before 'last', each after what separates it from
    // the previous child. 'afterComma' is what follows a comma.
    void appendChildren(vector<Layout>& parts, const Node& node, size_t first, size_t last, const Layout& afterComma)
    {
        for (size_t i = first; i < last; ++i)
        {
            if (i > 0)
            {
                parts.push_back(separator(node.children[i - 1], node.children[i], afterComma));
            }
            parts.push_back(print(node.children[i]));
        }
    }

    // The directives of a metadata list, in its brackets, without the comments before it. They are on one line when
    // they fit, and each one starts a line after the first otherwise.
    Layout directives(const Node& metadata)
    {
        vector<Layout> items;
        appendChildren(items, metadata, 1, metadata.children.size(), line());
        return listGroup(metadata.children.front().token, concat(std::move(items)));
    }

    Layout metadata(const Node& node) { return concat(beforeText(firstToken(node)), directives(node)); }

    // The comments that leave definitions as written: 'ignore' the one it leads, and 'off' the ones from
    // the one it leads to the one before the one that 'on' leads, or to the end of their body or file.
    constexpr string_view ignoreComment = "// ice-fmt: ignore";
    constexpr string_view offComment = "// ice-fmt: off";
    constexpr string_view onComment = "// ice-fmt: on";

    // Returns the index of the comment 'marker' among the comments and preprocessor directives that have lines of their
    // own before a node, if it is one of them.
    optional<size_t> markerIndex(const Node& node, string_view marker)
    {
        const vector<Trivia>& leading = firstToken(node).leading;
        for (size_t i = 0; i < leading.size(); ++i)
        {
            if (leading[i].ownLine && withoutTrailingBlanks(leading[i].text) == marker)
            {
                return i;
            }
        }
        return nullopt;
    }

    // The nodes from 'first' to 'last' as written, after the comments that lead 'first' up to the one at 'marker'. The
    // text goes on lines of its own, as a comment does, so its lines keep their indentation relative to the first one.
    Layout writtenText(const Node& first, const Node& last, size_t marker)
    {
        const Token& token = firstToken(first);
        bool afterMarker = marker + 1 < token.leading.size();
        const char* start = afterMarker ? token.leading[marker + 1].text.data() : token.text.data();
        int column = afterMarker ? token.leading[marker + 1].column : token.column;
        bool blankLine = afterMarker ? token.leading[marker + 1].blankLineBefore : token.blankLineBefore;

        const Token& lastOfLast = lastToken(last);
        string_view end = lastOfLast.trailing.empty() ? lastOfLast.text : lastOfLast.trailing.back().text;
        string_view written{start, static_cast<size_t>(end.data() + end.size() - start)};

        vector<Trivia> trivia{token.leading.begin(), token.leading.begin() + static_cast<ptrdiff_t>(marker) + 1};
        trivia.push_back({.text = written, .column = column, .ownLine = true, .blankLineBefore = blankLine});
        return leadingTrivia(trivia);
    }

    // The children that a comment leaves as written, from 'children[first]', and the index of the child after them.
    // 'end' is the index after the last child that formatting can leave off.
    struct AsWritten
    {
        Layout layout;
        size_t next;
    };

    optional<AsWritten> asWritten(const vector<Node>& children, size_t first, size_t end)
    {
        size_t next = first + 1;
        optional<size_t> marker = markerIndex(children[first], ignoreComment);
        if (!marker)
        {
            marker = markerIndex(children[first], offComment);
            if (!marker)
            {
                return nullopt;
            }
            while (next < end && !markerIndex(children[next], onComment))
            {
                ++next;
            }
        }
        return AsWritten{writtenText(children[first], children[next - 1], *marker), next};
    }

    Layout file(const Node& node)
    {
        vector<Layout> parts;
        for (size_t i = 0; i < node.children.size(); ++i)
        {
            // The EndOfFile token is only there for the comments that end the file.
            const Node& child = node.children[i];
            bool endOfFile = child.kind == NodeKind::Token;
            if (endOfFile && child.token.leading.empty())
            {
                break;
            }

            if (i > 0)
            {
                parts.push_back(hardline());
                if (blankLineBetween(node.children[i - 1], child))
                {
                    parts.push_back(hardline());
                }
            }
            if (endOfFile)
            {
                parts.push_back(leadingTrivia(child.token.leading));
            }
            else if (optional<AsWritten> written = asWritten(node.children, i, node.children.size() - 1))
            {
                parts.push_back(std::move(written->layout));
                i = written->next - 1;
            }
            else
            {
                parts.push_back(print(child));
            }
        }

        if (!parts.empty())
        {
            parts.push_back(hardline());
        }
        return concat(std::move(parts));
    }

    // An 'extends' or 'implements' clause, or an exception specification, after a line break that belongs to the
    // enclosing group. Its names are on one line when they fit, and each one starts a line after the first otherwise.
    Layout clause(const Node& node)
    {
        vector<Layout> names;
        appendChildren(names, node, 1, node.children.size(), line());
        return indent(concat(line(), print(node.children.front()), group(indent(concat(std::move(names))))));
    }

    // Lays out the 'count' metadata lists that lead a node, then 'rest', which is what follows them. As rustfmt does
    // with attributes, several lists go one per line, and the declaration of a container or an operation starts the
    // line after its metadata. So does a node that the author put there. Any other node follows its metadata list on
    // the same line when it fits there with the list on one line.
    Layout afterLeadingMetadata(const Node& node, size_t count, Layout rest)
    {
        if (count == 0)
        {
            return rest;
        }

        if (count > 1)
        {
            vector<Layout> parts;
            for (size_t i = 0; i < count; ++i)
            {
                parts.push_back(metadata(node.children[i]));
                parts.push_back(hardline());
            }
            parts.push_back(std::move(rest));
            return concat(std::move(parts));
        }

        // The metadata list shares a group with the node, so a list that breaks puts the node on the next line.
        const Node& list = node.children.front();
        bool ownLine = declaresContainer(node) || node.kind == NodeKind::Operation || startsLine(node.children[1]);
        return concat(
            beforeText(firstToken(list)),
            group(concat(directives(list), ownLine ? hardline() : line(), std::move(rest))));
    }

    // An operation that doesn't fit on one line is broken in two steps, the second one only when the first one is not
    // enough: its exception specification takes a line of its own, then each parameter takes one. The parameters are a
    // group inside the group of the exception specification.
    Layout operation(const Node& node)
    {
        size_t first = leadingMetadata(node);

        // The signature starts with 'idempotent' or the return type. The comments before it stay out of its group,
        // which they would break.
        const Node& start = node.children[first];
        vector<Layout> tokens;
        tokens.push_back(textAndTrailingTrivia(start.token));
        for (size_t i = first + 1; i < node.children.size(); ++i)
        {
            const Node& child = node.children[i];
            if (needsSpace(node.children[i - 1], child))
            {
                tokens.push_back(space());
            }
            tokens.push_back(child.kind == NodeKind::Clause ? clause(child) : print(child));
        }
        Layout signature = concat(beforeText(start.token), group(concat(std::move(tokens))));
        return afterLeadingMetadata(node, first, std::move(signature));
    }

    // The comments of the semicolon that follows a body, which is optional and is not written. They are laid out as
    // they read without it.
    Layout commentsOfOptionalSemicolon(const Node& body, const Node& semicolon)
    {
        const Token& token = semicolon.token;
        vector<Trivia> leading = token.leading;
        if (token.startsLine)
        {
            // Nothing else is left on the line of the semicolon, so each comment around it has a line of its own.
            leading.insert(leading.end(), token.trailing.begin(), token.trailing.end());
            for (Trivia& comment : leading)
            {
                comment.ownLine = true;
            }
            return leadingTrivia(leading);
        }

        vector<Layout> parts;
        if (needsSpace(body, semicolon))
        {
            parts.push_back(space());
        }
        parts.push_back(leadingTrivia(leading));
        parts.push_back(trailingTrivia(token));
        return concat(std::move(parts));
    }

    Layout contained(const Node& node)
    {
        // Only an optional semicolon follows a body.
        size_t count = node.children.size();
        bool optionalSemicolon = count > 1 && node.children[count - 2].kind == NodeKind::Body;

        size_t first = leadingMetadata(node);
        vector<Layout> parts;
        parts.push_back(print(node.children[first]));
        appendChildren(parts, node, first + 1, optionalSemicolon ? count - 1 : count, space());
        if (optionalSemicolon)
        {
            parts.push_back(commentsOfOptionalSemicolon(node.children[count - 2], node.children.back()));
        }
        return afterLeadingMetadata(node, first, concat(std::move(parts)));
    }

    // The parameters of an operation, or the types of a sequence or dictionary, are on one line when they fit, and each
    // one has its own line after the opening token otherwise.
    Layout list(const Node& node)
    {
        vector<Layout> items;
        if (node.children.size() > 2)
        {
            items.push_back(softline());
        }
        appendChildren(items, node, 1, node.children.size(), line());
        const Token& open = node.children.front().token;
        return concat(beforeText(open), listGroup(open, concat(std::move(items))));
    }

    // The definitions in a body, each after a line break, and each comma after its enumerator. No blank line follows
    // the opening brace.
    Layout definitions(const Node& node)
    {
        vector<Layout> parts;
        for (size_t i = 1; i < node.children.size() - 1; ++i)
        {
            const Node& previous = node.children[i - 1];
            const Node& child = node.children[i];
            if (isToken(child, TokenKind::Comma))
            {
                if (needsSpace(previous, child))
                {
                    parts.push_back(space());
                }
            }
            else
            {
                parts.push_back(line());
                if (i > 1 && blankLineBetween(previous, child))
                {
                    parts.push_back(hardline());
                }
            }

            if (optional<AsWritten> written = asWritten(node.children, i, node.children.size() - 1))
            {
                parts.push_back(std::move(written->layout));
                i = written->next - 1;
            }
            else
            {
                parts.push_back(print(child));
            }
        }
        return concat(std::move(parts));
    }

    // The comments before the closing brace of a body, indented like the definitions.
    Layout commentsBeforeClose(const Node& node, bool broken)
    {
        const vector<Trivia>& leading = node.children.back().token.leading;
        if (leading.empty())
        {
            return concat();
        }

        if (!broken)
        {
            // The comments of a body written on one line follow the definitions on that line, or have a line each when
            // the body doesn't fit on one.
            vector<Layout> parts;
            for (const Trivia& comment : leading)
            {
                parts.push_back(line());
                parts.push_back(text(string{comment.text}));
            }
            return concat(std::move(parts));
        }

        // The brace has a line of its own, so each of these comments has one too.
        vector<Trivia> comments = leading;
        for (Trivia& comment : comments)
        {
            comment.ownLine = true;
        }
        vector<Layout> parts;
        parts.push_back(line());
        if (node.children.size() > 2 && comments.front().blankLineBefore)
        {
            parts.push_back(hardline());
        }
        parts.push_back(leadingTrivia(comments));
        return concat(std::move(parts));
    }

    Layout body(const Node& node)
    {
        const Token& open = node.children.front().token;
        const Token& close = node.children.back().token;

        // A definition stays on a single line only when the author wrote all of it on one. An empty body written as {}
        // is the exception: it follows its definition on the same line.
        bool empty = node.children.size() == 2 && close.leading.empty();
        bool broken = close.line > open.line || (open.startsLine && !empty);

        return group(
            concat(
                line(),
                print(open),
                indent(concat(definitions(node), commentsBeforeClose(node, broken))),
                empty ? softline() : line(),
                textAndTrailingTrivia(close)),
            broken);
    }

    Layout print(const Node& node)
    {
        switch (node.kind)
        {
            case NodeKind::Token:
                return print(node.token);
            case NodeKind::File:
                return file(node);
            case NodeKind::Contained:
                return contained(node);
            case NodeKind::Operation:
                return operation(node);
            case NodeKind::Metadata:
                return metadata(node);
            case NodeKind::Parameters:
            case NodeKind::Types:
                return list(node);
            case NodeKind::Clause:
                // An 'extends' or 'implements' clause moves to the next line when it doesn't fit on the current one.
                return group(clause(node));
            case NodeKind::Body:
                return body(node);
        }
        assert(false);
        return {};
    }

    // Returns the lines of a comment or preprocessor directive without the blanks around each one, which formatting
    // manages.
    string withoutBlanks(string_view text)
    {
        constexpr string_view blanks = " \t\r\f\v";
        string result;
        while (true)
        {
            size_t end = text.find('\n');
            string_view line = text.substr(0, end);
            size_t first = line.find_first_not_of(blanks);
            if (first != string_view::npos)
            {
                result += line.substr(first, line.find_last_not_of(blanks) + 1 - first);
            }

            if (end == string_view::npos)
            {
                return result;
            }
            result += '\n';
            text.remove_prefix(end + 1);
        }
    }

    // What formatting keeps of a text: the tokens and preprocessor directives, in order, the doc comments with their
    // places, which decide what the compiler documents, and the other comments, in any order. The optional semicolon
    // after a closing brace is left out, since formatting removes it.
    struct Content
    {
        vector<string> code;

        // The place of a doc comment is the size of the code before its line: the compiler's preprocessor moves a
        // comment above its line, and a block comment joins the lines it spans.
        vector<pair<size_t, string>> docComments;

        vector<string> comments;

        bool operator==(const Content&) const = default;
    };

    Content contentOf(string_view text)
    {
        Content content;
        bool afterClosingBrace = false;

        // The line where the previous lexeme ends, and the size of the code where the preprocessor's current line
        // starts.
        int endLine = 0;
        size_t lineStart = 0;

        for (const Lexeme& lexeme : scan(text))
        {
            if (lexeme.line > endLine)
            {
                lineStart = content.code.size();
            }
            endLine = lexeme.line + static_cast<int>(ranges::count(lexeme.text, '\n'));

            switch (lexeme.kind)
            {
                case TokenKind::LineComment:
                case TokenKind::BlockComment:
                    if (isDocComment(lexeme))
                    {
                        content.docComments.emplace_back(lineStart, withoutBlanks(lexeme.text));
                    }
                    else
                    {
                        content.comments.push_back(withoutBlanks(lexeme.text));
                    }
                    break;
                case TokenKind::PreprocessorDirective:
                    content.code.push_back(withoutBlanks(lexeme.text));
                    break;
                default:
                    if (!afterClosingBrace || lexeme.kind != TokenKind::Semicolon)
                    {
                        content.code.emplace_back(lexeme.text);
                    }
                    afterClosingBrace = lexeme.kind == TokenKind::RightBrace;
                    break;
            }
        }
        ranges::sort(content.comments);
        return content;
    }
}

string
IceFmt::formatSlice(string_view text)
{
    // A byte order mark belongs to the file, not to its Slice text, and is kept.
    constexpr string_view byteOrderMark = "\xEF\xBB\xBF";
    string_view file = text;
    string formatted;
    if (text.starts_with(byteOrderMark))
    {
        formatted = byteOrderMark;
        text.remove_prefix(byteOrderMark.size());
    }

    auto ignoresFile = [](const Lexeme& lexeme)
    {
        return lexeme.kind == TokenKind::LineComment && withoutTrailingBlanks(lexeme.text) == "// ice-fmt: ignore-file";
    };
    if (ranges::any_of(scan(text), ignoresFile))
    {
        return string{file};
    }

    // The file keeps the line endings its first line has.
    bool windowsLineEndings = text.substr(0, text.find('\n')).ends_with('\r');

    // Output that loses or changes what the Slice compiler reads is a bug in the formatter, and the file stays as it
    // is.
    string rendered = render(print(parse(text)));
    if (contentOf(rendered) != contentOf(text))
    {
        throw logic_error{"formatting would change the tokens or comments"};
    }

    for (char c : rendered)
    {
        if (c == '\n' && windowsLineEndings)
        {
            formatted += '\r';
        }
        formatted += c;
    }
    return formatted;
}
