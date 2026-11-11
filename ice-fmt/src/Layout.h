// Copyright (c) ZeroC, Inc.

#ifndef SLICE_FORMAT_LAYOUT_H
#define SLICE_FORMAT_LAYOUT_H

#include <string>
#include <vector>

namespace SliceFormat
{
    /// A document to lay out: text, and the places where a line can end. The documents follow Philip Wadler's
    /// "A prettier printer", with the additions Prettier made for comments.
    struct Doc
    {
        enum class Kind
        {
            Text,
            Space,
            Line,
            HardLine,
            Suffix,
            EndOfLine,
            Indent,
            Group,
            Concat,
        };

        Kind kind;

        /// The text of a Text or Suffix, and what a Line becomes when its group is flat.
        std::string text;

        /// The child of an Indent or Group, and the parts of a Concat.
        std::vector<Doc> children;

        /// Whether a Group is broken even when it fits on the line.
        bool broken = false;

        /// The zero-based column, in bytes, where a Suffix starts when the line doesn't reach it.
        int column = 0;
    };

    /// Creates text that holds no line break.
    [[nodiscard]] Doc text(std::string text);

    /// Creates a space, which is left out at the start and at the end of a line.
    [[nodiscard]] Doc space();

    /// Creates a line break that becomes a space when the enclosing group is flat.
    [[nodiscard]] Doc line();

    /// Creates a line break that becomes nothing when the enclosing group is flat.
    [[nodiscard]] Doc softline();

    /// Creates a line break that breaks the enclosing groups.
    [[nodiscard]] Doc hardline();

    /// Creates text that ends its line and doesn't count toward the width of the line, for a comment that follows
    /// code.
    /// @param text The text.
    /// @param column The zero-based column, in bytes, where the text starts. When the line already reaches this
    /// column, a single space separates the text from it.
    [[nodiscard]] Doc suffix(std::string text, int column);

    /// Creates the end of the current line: what follows starts on a new line, unless the current line is empty.
    [[nodiscard]] Doc endOfLine();

    /// Indents by one level the lines that start within @p doc.
    [[nodiscard]] Doc indent(Doc doc);

    /// Creates a group, which is laid out flat when it fits on the line, and broken otherwise: each line break
    /// directly in a broken group ends the line.
    /// @param doc The contents of the group.
    /// @param broken Whether the group is broken even when it fits on the line.
    [[nodiscard]] Doc group(Doc doc, bool broken = false);

    /// Concatenates documents.
    [[nodiscard]] Doc concat(std::vector<Doc> docs);

    /// Lays out a document in lines of at most 120 columns where its line breaks allow, with four spaces for each
    /// level of indentation.
    [[nodiscard]] std::string render(const Doc& doc);
}

#endif
