// Copyright (c) ZeroC, Inc.

#ifndef ICE_FMT_LAYOUT_H
#define ICE_FMT_LAYOUT_H

#include <concepts>
#include <memory>
#include <string>
#include <utility>
#include <variant>
#include <vector>

namespace IceFmt
{
    /// Text to lay out, and the places where a line can end: what Philip Wadler's "A prettier printer" calls a
    /// document, with the additions Prettier made for comments. A layout doesn't change once made, so its copies share
    /// the layouts it holds.
    struct Layout
    {
        struct Text
        {
            std::string text;
        };

        struct Space
        {
        };

        struct Line
        {
            /// Whether the line break becomes nothing, rather than a space, when its group is flat.
            bool soft;
        };

        struct HardLine
        {
            /// Whether the next line starts at the first column, without indentation.
            bool literal;
        };

        struct Suffix
        {
            std::string text;
        };

        struct EndOfLine
        {
        };

        struct Indent
        {
            std::shared_ptr<const Layout> contents;
        };

        struct Group
        {
            std::shared_ptr<const Layout> contents;

            /// Whether the group is broken even when it fits on the line.
            bool broken;
        };

        struct Concat
        {
            std::vector<Layout> parts;
        };

        std::variant<Text, Space, Line, HardLine, Suffix, EndOfLine, Indent, Group, Concat> value;
    };

    /// Creates text that holds no line break.
    [[nodiscard]] Layout text(std::string text);

    /// Creates a space, which is left out at the start and at the end of a line.
    [[nodiscard]] Layout space();

    /// Creates a line break that becomes a space when the enclosing group is flat.
    [[nodiscard]] Layout line();

    /// Creates a line break that becomes nothing when the enclosing group is flat.
    [[nodiscard]] Layout softline();

    /// Creates a line break that breaks the enclosing groups.
    [[nodiscard]] Layout hardline();

    /// Creates a line break that breaks the enclosing groups, and starts the next line at the first column. The text
    /// that follows holds its own indentation.
    [[nodiscard]] Layout literalline();

    /// Creates text that ends its line and doesn't count toward the width of the line, for a comment that follows
    /// code. The suffixes of consecutive lines start in the same column, one space after the longest code of those
    /// lines.
    [[nodiscard]] Layout suffix(std::string text);

    /// Creates the end of the current line: what follows starts on a new line, unless the current line is empty.
    [[nodiscard]] Layout endOfLine();

    /// Indents by one level the lines that start within @p layout.
    [[nodiscard]] Layout indent(Layout layout);

    /// Creates a group, which is laid out flat when it fits on the line, and broken otherwise: each line break
    /// directly in a broken group ends the line.
    /// @param layout The contents of the group.
    /// @param broken Whether the group is broken even when it fits on the line.
    [[nodiscard]] Layout group(Layout layout, bool broken = false);

    /// Concatenates layouts.
    [[nodiscard]] Layout concat(std::vector<Layout> parts);

    /// Concatenates the layouts passed as arguments.
    template<std::same_as<Layout>... Layouts> [[nodiscard]] Layout concat(Layouts... layouts)
    {
        std::vector<Layout> parts;
        parts.reserve(sizeof...(layouts));
        (parts.push_back(std::move(layouts)), ...);
        return concat(std::move(parts));
    }

    /// Renders a layout in lines of at most 120 columns where its line breaks allow, with four spaces for each level
    /// of indentation.
    [[nodiscard]] std::string render(const Layout& layout);
}

#endif
