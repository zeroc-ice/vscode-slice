// Copyright (c) ZeroC, Inc.

#include "Layout.h"

#include <algorithm>
#include <cstddef>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <variant>
#include <vector>

using namespace std;
using namespace IceFmt;

namespace
{
    constexpr int lineWidth = 120;
    constexpr int indentWidth = 4;

    // A visitor made of one lambda for each kind of layout.
    template<typename... Lambdas> struct Overloaded : Lambdas...
    {
        using Lambdas::operator()...;
    };

    // Returns the number of columns the text takes: one for each UTF-8 code point.
    int columns(string_view text)
    {
        auto startsCodePoint = [](char c) { return (static_cast<unsigned char>(c) & 0xC0) != 0x80; };
        return static_cast<int>(ranges::count_if(text, startsCodePoint));
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
                visit(
                    Overloaded{
                        [&](const Layout::Text& text) { writeText(text.text, command.indent); },
                        [&](const Layout::Space&) { writeSpace(); },
                        [&](const Layout::Line& line)
                        {
                            if (!command.flat)
                            {
                                startLine();
                            }
                            else if (!line.soft)
                            {
                                writeSpace();
                            }
                        },
                        [&](const Layout::HardLine&) { startLine(); },
                        [&](const Layout::Suffix& suffix) { writeSuffix(suffix.text); },
                        [&](const Layout::EndOfLine&) { _endOfLine = !lineIsEmpty(); },
                        [&](const Layout::Indent& indent)
                        { commands.push_back({command.indent + indentWidth, command.flat, indent.contents.get()}); },
                        [&](const Layout::Group& group)
                        {
                            Command contents{command.indent, true, group.contents.get()};
                            contents.flat = command.flat || (!group.broken && fits(contents, commands));
                            commands.push_back(contents);
                        },
                        [&](const Layout::Concat& concat)
                        {
                            for (auto part = concat.parts.rbegin(); part != concat.parts.rend(); ++part)
                            {
                                commands.push_back({command.indent, command.flat, &*part});
                            }
                        }},
                    command.layout->value);
            }
            return alignSuffixes();
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

        // The suffixes of a line: the offset in _out where the code before them ends, and the columns that code takes.
        struct LineSuffix
        {
            int line;
            size_t codeEnd;
            int codeColumns;
        };

        [[nodiscard]] bool lineIsEmpty() const { return _out.size() == _lineStart; }

        void trimLine()
        {
            while (!lineIsEmpty() && _out.back() == ' ')
            {
                _out.pop_back();
            }
        }

        void startLine()
        {
            trimLine();
            _out += '\n';
            _lineStart = _out.size();
            _line++;
            _endOfLine = false;
        }

        // Writes a suffix one space after the code of its line. alignSuffixes moves it to its column.
        void writeSuffix(string_view text)
        {
            trimLine();
            if (_suffixes.empty() || _suffixes.back().line != _line)
            {
                _suffixes.push_back({_line, _out.size(), columns(string_view{_out}.substr(_lineStart))});
            }
            _out += ' ';
            _out += text;
            _endOfLine = true;
        }

        // Returns the rendered text with the suffixes of consecutive lines in one column, one space after the longest
        // code of those lines, as gofmt aligns trailing comments.
        [[nodiscard]] string alignSuffixes() const
        {
            string aligned;
            size_t copied = 0;
            size_t first = 0;
            while (first < _suffixes.size())
            {
                int codeColumns = _suffixes[first].codeColumns;
                size_t last = first + 1;
                while (last < _suffixes.size() && _suffixes[last].line == _suffixes[last - 1].line + 1)
                {
                    codeColumns = max(codeColumns, _suffixes[last].codeColumns);
                    last++;
                }

                for (size_t i = first; i < last; ++i)
                {
                    aligned.append(_out, copied, _suffixes[i].codeEnd - copied);
                    aligned.append(static_cast<size_t>(codeColumns - _suffixes[i].codeColumns), ' ');
                    copied = _suffixes[i].codeEnd;
                }
                first = last;
            }
            aligned.append(_out, copied);
            return aligned;
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

                // A layout that settles whether the contents fit returns the answer, and the others return nothing.
                optional<bool> answer = visit(
                    Overloaded{
                        [&](const Layout::Text& text) -> optional<bool>
                        {
                            if (text.text.empty())
                            {
                                return nullopt;
                            }
                            if (endOfLine)
                            {
                                // The text starts a new line, which the flat contents can't do.
                                return !command.flat;
                            }
                            remaining -= columns(text.text);
                            return nullopt;
                        },
                        [&](const Layout::Space&) -> optional<bool>
                        {
                            remaining--;
                            return nullopt;
                        },
                        [&](const Layout::Line& line) -> optional<bool>
                        {
                            if (!command.flat)
                            {
                                return true;
                            }
                            if (endOfLine)
                            {
                                return false;
                            }
                            if (!line.soft)
                            {
                                remaining--;
                            }
                            return nullopt;
                        },
                        [&](const Layout::HardLine&) -> optional<bool> { return !command.flat; },
                        [&](const Layout::Suffix&) -> optional<bool>
                        {
                            endOfLine = true;
                            return nullopt;
                        },
                        [&](const Layout::EndOfLine&) -> optional<bool>
                        {
                            endOfLine = true;
                            return nullopt;
                        },
                        [&](const Layout::Indent& indent) -> optional<bool>
                        {
                            commands.push_back({command.indent, command.flat, indent.contents.get()});
                            return nullopt;
                        },
                        [&](const Layout::Group& group) -> optional<bool>
                        {
                            if (command.flat && group.broken)
                            {
                                return false;
                            }
                            commands.push_back({command.indent, command.flat, group.contents.get()});
                            return nullopt;
                        },
                        [&](const Layout::Concat& concat) -> optional<bool>
                        {
                            for (auto part = concat.parts.rbegin(); part != concat.parts.rend(); ++part)
                            {
                                commands.push_back({command.indent, command.flat, &*part});
                            }
                            return nullopt;
                        }},
                    command.layout->value);
                if (answer)
                {
                    return *answer;
                }
            }
            return false;
        }

        string _out;

        // The offset in _out of the line being written, and its zero-based number.
        size_t _lineStart = 0;
        int _line = 0;

        // Whether the current line is complete: the next text starts a new line.
        bool _endOfLine = false;

        // The suffixes of each line that has any, in order.
        vector<LineSuffix> _suffixes;
    };
}

Layout
IceFmt::text(string text)
{
    return {Layout::Text{std::move(text)}};
}

Layout
IceFmt::space()
{
    return {Layout::Space{}};
}

Layout
IceFmt::line()
{
    return {Layout::Line{.soft = false}};
}

Layout
IceFmt::softline()
{
    return {Layout::Line{.soft = true}};
}

Layout
IceFmt::hardline()
{
    return {Layout::HardLine{}};
}

Layout
IceFmt::suffix(string text)
{
    return {Layout::Suffix{std::move(text)}};
}

Layout
IceFmt::endOfLine()
{
    return {Layout::EndOfLine{}};
}

Layout
IceFmt::indent(Layout layout)
{
    return {Layout::Indent{make_shared<const Layout>(std::move(layout))}};
}

Layout
IceFmt::group(Layout layout, bool broken)
{
    return {Layout::Group{.contents = make_shared<const Layout>(std::move(layout)), .broken = broken}};
}

Layout
IceFmt::concat(vector<Layout> parts)
{
    return {Layout::Concat{std::move(parts)}};
}

string
IceFmt::render(const Layout& layout)
{
    return Renderer{}.render(layout);
}
