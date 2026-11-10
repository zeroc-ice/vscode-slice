// Copyright (c) ZeroC, Inc.

#include "Formatter.h"
#include "Layout.h"
#include "SyntaxParser.h"

#include <algorithm>
#include <array>
#include <cassert>

using namespace std;
using namespace SliceFormat;

namespace
{
    // The tokens that no space follows, and the tokens that no space precedes.
    constexpr array<TokenKind, 4> noSpaceAfter{
        TokenKind::LeftParenthesis,
        TokenKind::LeftAngle,
        TokenKind::LeftBracket,
        TokenKind::DoubleLeftBracket};
    constexpr array<TokenKind, 9> noSpaceBefore{
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

    // Splits a comment or directive into its lines. A line after the first one keeps the indentation it has relative
    // to the first one.
    vector<string> splitLines(const Trivia& trivia)
    {
        auto column = static_cast<size_t>(trivia.column);
        vector<string> lines;
        size_t start = 0;
        while (start != string_view::npos)
        {
            size_t end = trivia.text.find('\n', start);
            string_view current = trivia.text.substr(start, end == string_view::npos ? end : end - start);
            start = end == string_view::npos ? end : end + 1;

            current = current.substr(0, current.find_last_not_of(" \t\r") + 1);
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
        }
        return lines;
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

    // The text of a token and the comments that follow it on its line.
    Doc textAndTrailingTrivia(const Token& token)
    {
        vector<Doc> docs{text(string{token.text})};
        for (const Trivia& trivia : token.trailing)
        {
            // The comment keeps its column, which often aligns it with the comments of the lines around it.
            docs.push_back(suffix(splitLines(trivia).front(), trivia.column));
        }
        return concat(std::move(docs));
    }

    // What precedes the text of a token: the comments and directives before it, then the blank line or the space
    // that separates the last one from the token.
    Doc beforeText(const Token& token)
    {
        vector<Doc> docs{leadingTrivia(token.leading)};
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

    Doc print(const Token& token) { return concat({beforeText(token), textAndTrailingTrivia(token)}); }

    // Groups a list that starts with the token 'open'. The comments before that token stay out of the group, which
    // they would break.
    Doc listGroup(const Token& open, Doc items)
    {
        return concat({beforeText(open), group(concat({textAndTrailingTrivia(open), indent(std::move(items))}))});
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
                    // The declaration stays on the line of its metadata only when the author wrote it there.
                    docs.push_back(startsLine(child) ? hardline() : space());
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
        for (const Node& child : node.children)
        {
            // The EndOfFile token is only there for the comments that end the file.
            bool endOfFile = child.kind == NodeKind::Token;
            if (endOfFile && child.token.leading.empty())
            {
                break;
            }

            if (!docs.empty())
            {
                docs.push_back(hardline());
                if (blankLineBefore(child))
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

    Doc declaration(const Node& node)
    {
        vector<Doc> docs;
        auto parameterList = ranges::find(node.children, NodeKind::Parameters, &Node::kind);
        if (parameterList == node.children.end())
        {
            appendChildren(docs, node, 0, node.children.size(), space());
            return concat(std::move(docs));
        }

        // The name of an operation starts a line when that keeps its parameters on one line.
        auto name = static_cast<size_t>(parameterList - node.children.begin()) - 1;
        appendChildren(docs, node, 0, name, space());
        vector<Doc> signature{print(node.children[name])};
        if (needsSpace(node.children[name], *parameterList))
        {
            signature.push_back(space());
        }
        signature.push_back(print(*parameterList));
        docs.push_back(lineIfFits(concat(std::move(signature))));
        appendChildren(docs, node, name + 2, node.children.size(), space());
        return concat(std::move(docs));
    }

    // The directives are on one line when they fit, and each one starts a line after the first otherwise.
    Doc metadata(const Node& node)
    {
        vector<Doc> directives;
        appendChildren(directives, node, 1, node.children.size(), line());
        return listGroup(node.children.front().token, concat(std::move(directives)));
    }

    // The parameters are on one line when they fit, and each one has its own line after the parenthesis otherwise.
    Doc parameters(const Node& node)
    {
        vector<Doc> items;
        if (node.children.size() > 2)
        {
            items.push_back(softline());
        }
        appendChildren(items, node, 1, node.children.size(), line());
        return listGroup(node.children.front().token, concat(std::move(items)));
    }

    // An 'extends' or 'throws' clause moves to the next line when it doesn't fit on the current one. Its names are on
    // one line when they fit, and each one starts a line after the first otherwise.
    Doc clause(const Node& node)
    {
        vector<Doc> names;
        appendChildren(names, node, 1, node.children.size(), line());
        return group(indent(concat({line(), print(node.children.front()), group(indent(concat(std::move(names))))})));
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

            // A blank line is kept between two members, not after the opening brace.
            members.push_back(line());
            if (i > 1 && blankLineBefore(child))
            {
                members.push_back(hardline());
            }
            members.push_back(print(child));
        }

        // A definition stays on a single line only when the author wrote all of it on one.
        bool broken = open.startsLine || close.line > open.line;

        // The comments before the closing brace are indented like the members. In a broken body the brace has a line
        // of its own, so each of these comments has one too.
        if (!close.leading.empty())
        {
            vector<Trivia> comments = close.leading;
            if (broken)
            {
                for (Trivia& comment : comments)
                {
                    comment.ownLine = true;
                }
            }

            members.push_back(line());
            if (last > 1 && comments.front().blankLineBefore)
            {
                members.push_back(hardline());
            }
            members.push_back(leadingTrivia(comments));
        }

        Doc beforeClose = members.empty() ? softline() : line();
        return group(
            concat(
                {line(), print(open), indent(concat(std::move(members))), beforeClose, textAndTrailingTrivia(close)}),
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
            case NodeKind::Declaration:
                return declaration(node);
            case NodeKind::Metadata:
                return metadata(node);
            case NodeKind::Parameters:
                return parameters(node);
            case NodeKind::Clause:
                return clause(node);
            case NodeKind::Body:
                return body(node);
        }
        assert(false);
        return {};
    }
}

string
SliceFormat::formatSlice(string_view text)
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
    size_t firstLineEnd = text.find('\n');
    bool windowsLineEndings = firstLineEnd != string_view::npos && firstLineEnd > 0 && text[firstLineEnd - 1] == '\r';

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
