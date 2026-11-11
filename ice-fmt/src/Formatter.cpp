// Copyright (c) ZeroC, Inc.

#include "Formatter.h"
#include "Layout.h"
#include "SyntaxParser.h"
#include "SyntaxScanner.h"

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
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

    Doc print(const Node& node);

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

    // Returns whether a blank line separates a node from the one before it: the author left one, or both are contained,
    // at least one of them is a container, and nothing separates them yet. A preprocessor directive between
    // the two leaves the choice to the author, since the directive can belong to either one.
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
        return (isContainer(previous) || isContainer(node)) && !separated &&
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

    // Returns a line of a comment or directive without the blanks and the carriage return that end it.
    string_view withoutTrailingBlanks(string_view line) { return line.substr(0, line.find_last_not_of(" \t\r") + 1); }

    // Splits a comment or directive into its lines. A line after the first one keeps the indentation it has relative
    // to the first one.
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

    // The comments and directives before a token, without the blank line that precedes the first one or the space
    // that follows the last one.
    Doc leadingTrivia(const vector<Trivia>& leading)
    {
        vector<Doc> docs;
        for (const Trivia& trivia : leading)
        {
            if (!docs.empty() && trivia.blankLineBefore)
            {
                // The first line break ends the previous comment.
                docs.push_back(hardline());
                docs.push_back(hardline());
            }

            if (trivia.ownLine)
            {
                vector<string> lines = splitLines(trivia);
                docs.push_back(endOfLine());
                docs.push_back(text(lines.front()));
                for (size_t i = 1; i < lines.size(); ++i)
                {
                    docs.push_back(hardline());
                    docs.push_back(text(lines[i]));
                }
                docs.push_back(endOfLine());
            }
            else
            {
                if (!docs.empty())
                {
                    docs.push_back(space());
                }
                docs.push_back(text(string{trivia.text}));
            }
        }
        return concat(std::move(docs));
    }

    // The comments that follow a token on its line.
    Doc trailingTrivia(const Token& token)
    {
        vector<Doc> docs;
        docs.reserve(token.trailing.size());
        for (const Trivia& trivia : token.trailing)
        {
            // The comment keeps its column, which often aligns it with the comments of the lines around it.
            docs.push_back(suffix(string{withoutTrailingBlanks(trivia.text)}, trivia.column));
        }
        return concat(std::move(docs));
    }

    // The text of a token and the comments that follow it on its line.
    Doc textAndTrailingTrivia(const Token& token) { return concat(text(string{token.text}), trailingTrivia(token)); }

    // What precedes the text of a token: the comments and directives before it, then the blank line or the space
    // that separates the last one from the token.
    Doc beforeText(const Token& token)
    {
        vector<Doc> docs;
        docs.push_back(leadingTrivia(token.leading));
        if (!token.leading.empty())
        {
            if (token.blankLineBefore)
            {
                docs.push_back(hardline());
                docs.push_back(hardline());
            }
            if (!token.leading.back().ownLine && !contains(noSpaceBefore, token.kind))
            {
                docs.push_back(space());
            }
        }
        return concat(std::move(docs));
    }

    Doc print(const Token& token) { return concat(beforeText(token), textAndTrailingTrivia(token)); }

    // Groups a list that starts with the token 'open'. The comments before that token stay out of the group, which
    // they would break.
    Doc listGroup(const Token& open, Doc items)
    {
        return concat(beforeText(open), group(concat(textAndTrailingTrivia(open), indent(std::move(items)))));
    }

    // Appends the children of a node from index 'first' to the one before 'last', each after what separates it from
    // the previous child. 'afterComma' is what follows a comma.
    void appendChildren(vector<Doc>& docs, const Node& node, size_t first, size_t last, const Doc& afterComma)
    {
        for (size_t i = first; i < last; ++i)
        {
            const Node& child = node.children[i];
            if (i > 0)
            {
                const Node& previous = node.children[i - 1];
                if (isToken(previous, TokenKind::Comma))
                {
                    docs.push_back(afterComma);
                }
                else if (previous.kind == NodeKind::Metadata)
                {
                    // Metadata stays on the line of the type or name it applies to. Two metadata lists keep the line
                    // break the author put between them.
                    bool ownLine = child.kind == NodeKind::Metadata && startsLine(child);
                    docs.push_back(ownLine ? hardline() : space());
                }
                else if (needsSpace(previous, child))
                {
                    docs.push_back(space());
                }
            }
            docs.push_back(print(child));
        }
    }

    Doc file(const Node& node)
    {
        vector<Doc> docs;
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
                docs.push_back(hardline());
                if (blankLineBetween(node.children[i - 1], child))
                {
                    docs.push_back(hardline());
                }
            }
            docs.push_back(endOfFile ? leadingTrivia(child.token.leading) : print(child));
        }

        if (!docs.empty())
        {
            docs.push_back(hardline());
        }
        return concat(std::move(docs));
    }

    // An 'extends' or 'throws' clause, after a line break that belongs to the enclosing group. Its names are on one
    // line when they fit, and each one starts a line after the first otherwise.
    Doc clause(const Node& node)
    {
        vector<Doc> names;
        appendChildren(names, node, 1, node.children.size(), line());
        return indent(concat(line(), print(node.children.front()), group(indent(concat(std::move(names))))));
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

    // Lays out the 'count' metadata lists that lead a node, then 'rest', which is what follows them. A container starts
    // the line after its metadata, and so does a node that the author put there. Any other node follows its metadata
    // on the same line when it fits there.
    Doc afterLeadingMetadata(const Node& node, size_t count, Doc rest)
    {
        if (count == 0)
        {
            return rest;
        }

        vector<Doc> docs;
        appendChildren(docs, node, 0, count, space());
        bool ownLine = isContainer(node) || startsLine(node.children[count]);
        docs.push_back(group(concat(ownLine ? hardline() : line(), std::move(rest))));
        return concat(std::move(docs));
    }

    // An operation that doesn't fit on one line is broken in three steps, each one only when the previous ones are
    // not enough: its metadata takes the line above, its throws clause takes a line of its own, then each parameter
    // takes one. Each step is a group inside the group of the previous step.
    Doc operation(const Node& node)
    {
        size_t first = leadingMetadata(node);

        // The signature starts with 'idempotent' or the return type. The comments before it stay out of its group,
        // which they would break.
        const Node& start = node.children[first];
        vector<Doc> tokens;
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
        Doc signature = concat(beforeText(start.token), group(concat(std::move(tokens))));
        return afterLeadingMetadata(node, first, std::move(signature));
    }

    // The comments of the semicolon that follows a body, which is optional and is not written. They are laid out as
    // they read without it.
    Doc commentsOfOptionalSemicolon(const Node& body, const Node& semicolon)
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

        vector<Doc> docs;
        if (needsSpace(body, semicolon))
        {
            docs.push_back(space());
        }
        docs.push_back(leadingTrivia(leading));
        for (const Trivia& comment : token.trailing)
        {
            // The comment moves left by the column the semicolon took.
            docs.push_back(suffix(string{withoutTrailingBlanks(comment.text)}, comment.column - 1));
        }
        return concat(std::move(docs));
    }

    Doc contained(const Node& node)
    {
        // Only an optional semicolon follows a body.
        size_t count = node.children.size();
        bool optionalSemicolon = count > 1 && node.children[count - 2].kind == NodeKind::Body;

        size_t first = leadingMetadata(node);
        vector<Doc> docs;
        docs.push_back(print(node.children[first]));
        appendChildren(docs, node, first + 1, optionalSemicolon ? count - 1 : count, space());
        if (optionalSemicolon)
        {
            docs.push_back(commentsOfOptionalSemicolon(node.children[count - 2], node.children.back()));
        }
        return afterLeadingMetadata(node, first, concat(std::move(docs)));
    }

    // The directives are on one line when they fit, and each one starts a line after the first otherwise.
    Doc metadata(const Node& node)
    {
        vector<Doc> directives;
        appendChildren(directives, node, 1, node.children.size(), line());
        return listGroup(node.children.front().token, concat(std::move(directives)));
    }

    // The parameters of an operation, or the types of a sequence or dictionary, are on one line when they fit, and each
    // one has its own line after the opening token otherwise.
    Doc list(const Node& node)
    {
        vector<Doc> items;
        if (node.children.size() > 2)
        {
            items.push_back(softline());
        }
        appendChildren(items, node, 1, node.children.size(), line());
        return listGroup(node.children.front().token, concat(std::move(items)));
    }

    Doc body(const Node& node)
    {
        const Token& open = node.children.front().token;
        const Token& close = node.children.back().token;
        size_t last = node.children.size() - 1;

        vector<Doc> members;
        for (size_t i = 1; i < last; ++i)
        {
            const Node& child = node.children[i];
            if (isToken(child, TokenKind::Comma))
            {
                if (needsSpace(node.children[i - 1], child))
                {
                    members.push_back(space());
                }
                members.push_back(print(child));
                continue;
            }

            // No blank line follows the opening brace.
            members.push_back(line());
            if (i > 1 && blankLineBetween(node.children[i - 1], child))
            {
                members.push_back(hardline());
            }
            members.push_back(print(child));
        }

        // A definition stays on a single line only when the author wrote all of it on one. An empty body written as {}
        // is the exception: it follows its definition on the same line.
        bool empty = last == 1 && close.leading.empty();
        bool broken = close.line > open.line || (open.startsLine && !empty);

        // The comments before the closing brace are indented like the members.
        if (broken && !close.leading.empty())
        {
            // The brace has a line of its own, so each of these comments has one too.
            vector<Trivia> comments = close.leading;
            for (Trivia& comment : comments)
            {
                comment.ownLine = true;
            }

            members.push_back(line());
            if (last > 1 && comments.front().blankLineBefore)
            {
                members.push_back(hardline());
            }
            members.push_back(leadingTrivia(comments));
        }
        else
        {
            // The comments of a body written on one line follow the members on that line, or have a line each when
            // the body doesn't fit on one.
            for (const Trivia& comment : close.leading)
            {
                members.push_back(line());
                members.push_back(text(string{comment.text}));
            }
        }

        Doc beforeClose = members.empty() ? softline() : line();
        return group(
            concat(
                line(),
                print(open),
                indent(concat(std::move(members))),
                std::move(beforeClose),
                textAndTrailingTrivia(close)),
            broken);
    }

    Doc print(const Node& node)
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
                // An 'extends' clause moves to the next line when it doesn't fit on the current one.
                return group(clause(node));
            case NodeKind::Body:
                return body(node);
        }
        assert(false);
        return {};
    }
}

string
IceFmt::formatSlice(string_view text)
{
    // A byte order mark belongs to the file, not to its Slice text, and is kept.
    constexpr string_view byteOrderMark = "\xEF\xBB\xBF";
    string formatted;
    if (text.starts_with(byteOrderMark))
    {
        formatted = byteOrderMark;
        text.remove_prefix(byteOrderMark.size());
    }

    // The file keeps the line endings its first line has.
    bool windowsLineEndings = text.substr(0, text.find('\n')).ends_with('\r');

    for (char c : render(print(parse(text))))
    {
        if (c == '\n' && windowsLineEndings)
        {
            formatted += '\r';
        }
        formatted += c;
    }
    return formatted;
}
