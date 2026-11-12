// Copyright (c) ZeroC, Inc.

#include "Layout.h"

#include <algorithm>
#include <cstddef>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

using namespace std;
using namespace IceFmt;

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

    Layout make(Layout::Kind kind, string text = {})
    {
        Layout layout;
        layout.kind = kind;
        layout.text = std::move(text);
        return layout;
    }

    class Renderer final
    {
    public:
        string render(const Layout& root)
        {
            vector<Command> commands{{0, false, &root}};
            while (!commands.empty())
            {
                Command command = commands.back();
                commands.pop_back();
                const Layout& layout = *command.layout;
                switch (layout.kind)
                {
                    case Layout::Kind::Text:
                        writeText(layout.text, command.indent);
                        break;
                    case Layout::Kind::Space:
                        writeSpace();
                        break;
                    case Layout::Kind::Line:
                        if (!command.flat)
                        {
                            startLine();
                        }
                        else if (!layout.text.empty())
                        {
                            writeSpace();
                        }
                        break;
                    case Layout::Kind::HardLine:
                        startLine();
                        break;
                    case Layout::Kind::Suffix:
                    {
                        size_t length = _out.size() - _lineStart;
                        auto column = static_cast<size_t>(layout.column);
                        _out.append(column > length ? column - length : 1, ' ');
                        _out += layout.text;
                        _endOfLine = true;
                        break;
                    }
                    case Layout::Kind::EndOfLine:
                        _endOfLine = !lineIsEmpty();
                        break;
                    case Layout::Kind::Indent:
                        commands.push_back({command.indent + indentWidth, command.flat, &layout.children.front()});
                        break;
                    case Layout::Kind::Group:
                    {
                        Command contents{command.indent, true, &layout.children.front()};
                        contents.flat = command.flat || (!layout.broken && fits(contents, commands));
                        commands.push_back(contents);
                        break;
                    }
                    case Layout::Kind::Concat:
                        for (auto child = layout.children.rbegin(); child != layout.children.rend(); ++child)
                        {
                            commands.push_back({command.indent, command.flat, &*child});
                        }
                        break;
                }
            }
            return std::move(_out);
        }

    private:
        // A layout to render, with the indentation of the lines that start within it, and whether its line breaks
        // are flat.
        struct Command
        {
            int indent;
            bool flat;
            const Layout* layout;
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
            }
            _out += text;
        }

        void writeSpace()
        {
            if (!lineIsEmpty() && _out.back() != ' ')
            {
                _out += ' ';
            }
        }

        // Returns whether the flat contents of a group fit on the current line, together with what follows the group
        // up to the next place where the line can end. 'rest' holds what follows, last in first out.
        [[nodiscard]] bool fits(Command contents, const vector<Command>& rest) const
        {
            // The contents start a new line when the current line is complete.
            int column = lineIsEmpty() || _endOfLine ? contents.indent : columns(string_view{_out}.substr(_lineStart));
            int remaining = lineWidth - column;
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
                const Layout& layout = *command.layout;
                switch (layout.kind)
                {
                    case Layout::Kind::Text:
                        if (layout.text.empty())
                        {
                            break;
                        }
                        if (endOfLine)
                        {
                            // The text starts a new line, which the flat contents can't do.
                            return !command.flat;
                        }
                        remaining -= columns(layout.text);
                        break;
                    case Layout::Kind::Space:
                        remaining--;
                        break;
                    case Layout::Kind::Line:
                        if (!command.flat)
                        {
                            return true;
                        }
                        if (endOfLine)
                        {
                            return false;
                        }
                        remaining -= columns(layout.text);
                        break;
                    case Layout::Kind::HardLine:
                        return !command.flat;
                    case Layout::Kind::Suffix:
                    case Layout::Kind::EndOfLine:
                        endOfLine = true;
                        break;
                    case Layout::Kind::Group:
                        if (command.flat && layout.broken)
                        {
                            return false;
                        }
                        [[fallthrough]];
                    case Layout::Kind::Indent:
                        commands.push_back({command.indent, command.flat, &layout.children.front()});
                        break;
                    case Layout::Kind::Concat:
                        for (auto child = layout.children.rbegin(); child != layout.children.rend(); ++child)
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

        // Whether the current line is complete: the next text starts a new line.
        bool _endOfLine = false;
    };
}

Layout
IceFmt::text(string text)
{
    return make(Layout::Kind::Text, std::move(text));
}

Layout
IceFmt::space()
{
    return make(Layout::Kind::Space);
}

Layout
IceFmt::line()
{
    return make(Layout::Kind::Line, " ");
}

Layout
IceFmt::softline()
{
    return make(Layout::Kind::Line);
}

Layout
IceFmt::hardline()
{
    return make(Layout::Kind::HardLine);
}

Layout
IceFmt::suffix(string text, int column)
{
    Layout result = make(Layout::Kind::Suffix, std::move(text));
    result.column = column;
    return result;
}

Layout
IceFmt::endOfLine()
{
    return make(Layout::Kind::EndOfLine);
}

Layout
IceFmt::indent(Layout layout)
{
    Layout result = make(Layout::Kind::Indent);
    result.children.push_back(std::move(layout));
    return result;
}

Layout
IceFmt::group(Layout layout, bool broken)
{
    Layout result = make(Layout::Kind::Group);
    result.children.push_back(std::move(layout));
    result.broken = broken;
    return result;
}

Layout
IceFmt::concat(vector<Layout> parts)
{
    Layout result = make(Layout::Kind::Concat);
    result.children = std::move(parts);
    return result;
}

string
IceFmt::render(const Layout& layout)
{
    return Renderer{}.render(layout);
}
