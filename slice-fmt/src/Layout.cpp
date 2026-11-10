// Copyright (c) ZeroC, Inc.

#include "Layout.h"

#include <algorithm>
#include <string_view>

using namespace std;
using namespace SliceFormat;

namespace
{
    constexpr int lineWidth = 120;
    constexpr int indentWidth = 4;

    // Returns the number of columns the text takes: one for each UTF-8 code point.
    int columns(string_view text)
    {
        auto startsCodePoint = [](char c) { return (static_cast<unsigned char>(c) & 0xC0) != 0x80; };
        return static_cast<int>(ranges::count_if(text, startsCodePoint));
    }

    Doc make(Doc::Kind kind, string text = {})
    {
        Doc doc;
        doc.kind = kind;
        doc.text = std::move(text);
        return doc;
    }

    class Renderer final
    {
    public:
        string render(const Doc& root)
        {
            vector<Command> commands{{0, false, &root}};
            while (!commands.empty())
            {
                Command command = commands.back();
                commands.pop_back();
                const Doc& doc = *command.doc;
                switch (doc.kind)
                {
                    case Doc::Kind::Text:
                        writeText(doc.text, command.indent);
                        break;
                    case Doc::Kind::Space:
                        writeSpace();
                        break;
                    case Doc::Kind::Line:
                        if (!command.flat)
                        {
                            startLine();
                        }
                        else if (!doc.text.empty())
                        {
                            writeSpace();
                        }
                        break;
                    case Doc::Kind::HardLine:
                        startLine();
                        break;
                    case Doc::Kind::Suffix:
                    {
                        size_t length = _out.size() - _lineStart;
                        auto column = static_cast<size_t>(doc.column);
                        _out.append(column > length ? column - length : 1, ' ');
                        _out += doc.text;
                        _endOfLine = true;
                        break;
                    }
                    case Doc::Kind::EndOfLine:
                        _endOfLine = !lineIsEmpty();
                        break;
                    case Doc::Kind::Indent:
                        commands.push_back({command.indent + indentWidth, command.flat, &doc.children.front()});
                        break;
                    case Doc::Kind::Group:
                    {
                        Command contents{command.indent, true, &doc.children.front()};
                        // The contents start a new line when the current line is complete.
                        int remaining = lineWidth - (lineIsEmpty() || _endOfLine ? contents.indent : _column);
                        contents.flat = command.flat || (!doc.broken && fits(contents, commands, remaining));
                        commands.push_back(contents);
                        break;
                    }
                    case Doc::Kind::LineIfFits:
                    {
                        // A new line is worth it only when the flat contents don't fit on this one and do on the next.
                        Command contents{command.indent, true, &doc.children.front()};
                        bool newLine = !command.flat && !lineIsEmpty() && !_endOfLine &&
                                       !fits(contents, commands, lineWidth - _column - 1) &&
                                       fits(contents, commands, lineWidth - contents.indent);
                        if (newLine)
                        {
                            startLine();
                        }
                        else
                        {
                            writeSpace();
                        }
                        contents.flat = command.flat;
                        commands.push_back(contents);
                        break;
                    }
                    case Doc::Kind::Concat:
                        for (auto child = doc.children.rbegin(); child != doc.children.rend(); ++child)
                        {
                            commands.push_back({command.indent, command.flat, &*child});
                        }
                        break;
                }
            }
            return std::move(_out);
        }

    private:
        // A document to lay out, with the indentation of the lines that start within it, and whether its line breaks
        // are flat.
        struct Command
        {
            int indent;
            bool flat;
            const Doc* doc;
        };

        [[nodiscard]] bool lineIsEmpty() const { return _out.size() == _lineStart; }

        void startLine()
        {
            while (!lineIsEmpty() && _out.back() == ' ')
            {
                _out.pop_back();
            }
            _out += '\n';
            _lineStart = _out.size();
            _column = 0;
            _endOfLine = false;
        }

        void writeText(string_view text, int indent)
        {
            if (text.empty())
            {
                return;
            }
            if (_endOfLine)
            {
                startLine();
            }
            if (lineIsEmpty())
            {
                _out.append(static_cast<size_t>(indent), ' ');
                _column = indent;
            }
            _out += text;
            _column += columns(text);
        }

        void writeSpace()
        {
            if (!lineIsEmpty() && _out.back() != ' ')
            {
                _out += ' ';
                _column++;
            }
        }

        // Returns whether flat contents fit in the remaining columns of a line, together with what follows them up
        // to the next place where the line can end. 'rest' holds what follows, last in first out.
        [[nodiscard]] static bool fits(Command contents, const vector<Command>& rest, int remaining)
        {
            bool endOfLine = false;
            vector<Command> commands{contents};
            auto next = rest.rbegin();
            while (remaining >= 0)
            {
                if (commands.empty())
                {
                    if (next == rest.rend())
                    {
                        return true;
                    }
                    commands.push_back(*next++);
                }

                Command command = commands.back();
                commands.pop_back();
                const Doc& doc = *command.doc;
                switch (doc.kind)
                {
                    case Doc::Kind::Text:
                        if (doc.text.empty())
                        {
                            break;
                        }
                        if (endOfLine)
                        {
                            // The text starts a new line, which the flat contents can't do.
                            return !command.flat;
                        }
                        remaining -= columns(doc.text);
                        break;
                    case Doc::Kind::Space:
                        remaining--;
                        break;
                    case Doc::Kind::LineIfFits:
                    {
                        Command child{command.indent, true, &doc.children.front()};
                        if (!command.flat)
                        {
                            // The line can end here when the child fits on the next one.
                            vector<Command> following{rest.begin(), next.base()};
                            following.insert(following.end(), commands.begin(), commands.end());
                            if (fits(child, following, lineWidth - command.indent))
                            {
                                return true;
                            }
                        }
                        remaining--;
                        child.flat = command.flat;
                        commands.push_back(child);
                        break;
                    }
                    case Doc::Kind::Line:
                        if (!command.flat)
                        {
                            return true;
                        }
                        if (endOfLine)
                        {
                            return false;
                        }
                        remaining -= columns(doc.text);
                        break;
                    case Doc::Kind::HardLine:
                        return !command.flat;
                    case Doc::Kind::Suffix:
                    case Doc::Kind::EndOfLine:
                        endOfLine = true;
                        break;
                    case Doc::Kind::Group:
                        if (command.flat && doc.broken)
                        {
                            return false;
                        }
                        [[fallthrough]];
                    case Doc::Kind::Indent:
                        commands.push_back({command.indent, command.flat, &doc.children.front()});
                        break;
                    case Doc::Kind::Concat:
                        for (auto child = doc.children.rbegin(); child != doc.children.rend(); ++child)
                        {
                            commands.push_back({command.indent, command.flat, &*child});
                        }
                        break;
                }
            }
            return false;
        }

        string _out;

        // The offset in _out of the line being written.
        size_t _lineStart = 0;

        // The number of columns written on the current line, without the suffix that ends it.
        int _column = 0;

        // Whether the current line is complete: the next text starts a new line.
        bool _endOfLine = false;
    };
}

Doc
SliceFormat::text(string text)
{
    return make(Doc::Kind::Text, std::move(text));
}

Doc
SliceFormat::space()
{
    return make(Doc::Kind::Space);
}

Doc
SliceFormat::line()
{
    return make(Doc::Kind::Line, " ");
}

Doc
SliceFormat::softline()
{
    return make(Doc::Kind::Line);
}

Doc
SliceFormat::hardline()
{
    return make(Doc::Kind::HardLine);
}

Doc
SliceFormat::suffix(string text, int column)
{
    Doc result = make(Doc::Kind::Suffix, std::move(text));
    result.column = column;
    return result;
}

Doc
SliceFormat::endOfLine()
{
    return make(Doc::Kind::EndOfLine);
}

Doc
SliceFormat::indent(Doc doc)
{
    Doc result = make(Doc::Kind::Indent);
    result.children.push_back(std::move(doc));
    return result;
}

Doc
SliceFormat::group(Doc doc, bool broken)
{
    Doc result = make(Doc::Kind::Group);
    result.children.push_back(std::move(doc));
    result.broken = broken;
    return result;
}

Doc
SliceFormat::lineIfFits(Doc doc)
{
    Doc result = make(Doc::Kind::LineIfFits);
    result.children.push_back(std::move(doc));
    return result;
}

Doc
SliceFormat::concat(vector<Doc> docs)
{
    Doc result = make(Doc::Kind::Concat);
    result.children = std::move(docs);
    return result;
}

string
SliceFormat::render(const Doc& doc)
{
    return Renderer{}.render(doc);
}
