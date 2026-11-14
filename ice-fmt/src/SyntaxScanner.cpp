// Copyright (c) ZeroC, Inc.

#include "SyntaxScanner.h"

#include <algorithm>
#include <array>
#include <cassert>
#include <cstddef>
#include <optional>
#include <string_view>
#include <utility>
#include <vector>

using namespace std;
using namespace IceFmt;

// The tokens are those of the Slice compilers' scanner, cpp/src/Slice/Scanner.l in zeroc-ice/ice. Each function below
// returns the offset where the lexeme that starts at 'start' ends, or 'start' when no such lexeme starts there.
namespace
{
    struct Spelling
    {
        string_view text;
        TokenKind kind;
    };

    constexpr auto keywords = to_array<Spelling>(
        {{"module", TokenKind::Module},
         {"class", TokenKind::Class},
         {"interface", TokenKind::Interface},
         {"exception", TokenKind::Exception},
         {"struct", TokenKind::Struct},
         {"sequence", TokenKind::Sequence},
         {"dictionary", TokenKind::Dictionary},
         {"enum", TokenKind::Enum},
         {"out", TokenKind::Out},
         {"extends", TokenKind::Extends},
         {"throws", TokenKind::Throws},
         {"void", TokenKind::Void},
         {"bool", TokenKind::Bool},
         {"byte", TokenKind::Byte},
         {"short", TokenKind::Short},
         {"int", TokenKind::Int},
         {"long", TokenKind::Long},
         {"float", TokenKind::Float},
         {"double", TokenKind::Double},
         {"string", TokenKind::String},
         {"Object", TokenKind::Object},
         {"Value", TokenKind::Value},
         {"const", TokenKind::Const},
         {"false", TokenKind::False},
         {"true", TokenKind::True},
         {"idempotent", TokenKind::Idempotent},
         {"optional", TokenKind::Optional}});

    // The two-character marks come first: the scanner takes the first mark that matches.
    constexpr auto punctuationMarks = to_array<Spelling>(
        {{"[[", TokenKind::DoubleLeftBracket},
         {"]]", TokenKind::DoubleRightBracket},
         {"[", TokenKind::LeftBracket},
         {"]", TokenKind::RightBracket},
         {"{", TokenKind::LeftBrace},
         {"}", TokenKind::RightBrace},
         {"(", TokenKind::LeftParenthesis},
         {")", TokenKind::RightParenthesis},
         {"<", TokenKind::LeftAngle},
         {">", TokenKind::RightAngle},
         {",", TokenKind::Comma},
         {";", TokenKind::Semicolon},
         {"=", TokenKind::Equals},
         {"*", TokenKind::Asterisk},
         {":", TokenKind::Colon}});

    bool isBlank(char c) { return c == ' ' || c == '\t'; }

    bool isWhitespace(char c) { return isBlank(c) || c == '\r' || c == '\f' || c == '\v'; }

    bool isDigit(char c) { return c >= '0' && c <= '9'; }

    bool isHexDigit(char c) { return isDigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'); }

    bool isNameStart(char c) { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'; }

    bool isNameChar(char c) { return isNameStart(c) || isDigit(c); }

    bool isNotAscii(char c) { return static_cast<unsigned char>(c) >= 0x80; }

    bool isAt(string_view text, size_t offset, char c) { return offset < text.size() && text[offset] == c; }

    // Returns the offset of the first character at or after 'start' that doesn't satisfy 'matches'.
    size_t skip(string_view text, size_t start, bool (*matches)(char))
    {
        size_t end = start;
        while (end < text.size() && matches(text[end]))
        {
            end++;
        }
        return end;
    }

    // A preprocessor directive or a line comment runs to the end of its line, or of its last continuation line.
    size_t continuedLineEnd(string_view text, size_t start)
    {
        size_t end = start;
        while (end < text.size() && text[end] != '\n')
        {
            if (text[end] == '\\')
            {
                // A backslash at the end of a line continues it on the next line.
                if (isAt(text, end + 1, '\n'))
                {
                    end += 1;
                }
                else if (isAt(text, end + 1, '\r') && isAt(text, end + 2, '\n'))
                {
                    end += 2;
                }
            }
            end++;
        }
        return end;
    }

    size_t blockCommentEnd(string_view text, size_t start)
    {
        size_t close = text.find("*/", start + 2);
        return close == string_view::npos ? start : close + 2;
    }

    // A string literal ends on the line where it starts.
    size_t stringLiteralEnd(string_view text, size_t start)
    {
        size_t end = start + 1;
        while (end < text.size() && text[end] != '\n')
        {
            if (text[end] == '"')
            {
                return end + 1;
            }
            // A backslash escapes the next character, unless it ends the line.
            if (text[end] == '\\' && !isAt(text, end + 1, '\n'))
            {
                end++;
            }
            end++;
        }
        return start;
    }

    size_t signEnd(string_view text, size_t start)
    {
        return isAt(text, start, '+') || isAt(text, start, '-') ? start + 1 : start;
    }

    // integer := sign? ('0x' hexDigit+ | digit+)
    size_t integerEnd(string_view text, size_t start)
    {
        size_t digits = signEnd(text, start);
        size_t end = skip(text, digits, isDigit);
        if (end == digits)
        {
            return start;
        }

        if (text[digits] == '0' && isAt(text, digits + 1, 'x'))
        {
            size_t hexEnd = skip(text, digits + 2, isHexDigit);
            if (hexEnd > digits + 2)
            {
                return hexEnd;
            }
        }
        return end;
    }

    // exponent := ('e' | 'E') sign? digit+
    size_t exponentEnd(string_view text, size_t start)
    {
        if (!isAt(text, start, 'e') && !isAt(text, start, 'E'))
        {
            return start;
        }
        size_t digits = signEnd(text, start + 1);
        size_t end = skip(text, digits, isDigit);
        return end == digits ? start : end;
    }

    // floatingPoint := sign? (digit* '.' digit+ exponent? | digit+ '.' exponent? | digit+ exponent) ('f' | 'F')?
    size_t floatingPointEnd(string_view text, size_t start)
    {
        size_t digits = signEnd(text, start);
        size_t end = skip(text, digits, isDigit);
        if (isAt(text, end, '.'))
        {
            size_t fractionEnd = skip(text, end + 1, isDigit);
            if (end == digits && fractionEnd == end + 1)
            {
                // A '.' with no digit on either side.
                return start;
            }
            end = exponentEnd(text, fractionEnd);
        }
        else
        {
            size_t mantissaEnd = end;
            end = exponentEnd(text, mantissaEnd);
            if (mantissaEnd == digits || end == mantissaEnd)
            {
                // Without a '.', the literal needs digits and an exponent.
                return start;
            }
        }
        return isAt(text, end, 'f') || isAt(text, end, 'F') ? end + 1 : end;
    }

    // identifier := ('::'? '\'? nameStart nameChar*)+
    size_t identifierEnd(string_view text, size_t start)
    {
        size_t end = start;
        while (true)
        {
            size_t name = text.substr(end).starts_with("::") ? end + 2 : end;
            if (isAt(text, name, '\\'))
            {
                name++;
            }
            if (name == text.size() || !isNameStart(text[name]))
            {
                return end;
            }
            end = skip(text, name, isNameChar);
        }
    }

    // A keyword is a name only when a backslash escapes it.
    TokenKind identifierKind(string_view identifier)
    {
        auto keyword = ranges::find(keywords, identifier, &Spelling::text);
        if (keyword != keywords.end())
        {
            return keyword->kind;
        }
        return identifier.find("::") == string_view::npos ? TokenKind::Identifier : TokenKind::ScopedIdentifier;
    }

    // Returns the kind of the lexeme that starts at 'start', or nullopt for whitespace, and the offset where it ends.
    pair<optional<TokenKind>, size_t> lexemeAt(string_view text, size_t start)
    {
        char c = text[start];
        if (c == '\n')
        {
            return {nullopt, start + 1};
        }
        if (isWhitespace(c))
        {
            return {nullopt, skip(text, start, isWhitespace)};
        }

        // A preprocessor directive starts with a '#' that only blanks precede on its line.
        if (c == '#')
        {
            size_t previous = text.substr(0, start).find_last_not_of(" \t");
            if (previous == string_view::npos || text[previous] == '\n')
            {
                return {TokenKind::PreprocessorDirective, continuedLineEnd(text, start)};
            }
        }
        if (c == '/' && isAt(text, start + 1, '/'))
        {
            return {TokenKind::LineComment, continuedLineEnd(text, start)};
        }
        if (c == '/' && isAt(text, start + 1, '*'))
        {
            if (size_t end = blockCommentEnd(text, start); end > start)
            {
                return {TokenKind::BlockComment, end};
            }
        }
        if (c == '"')
        {
            if (size_t end = stringLiteralEnd(text, start); end > start)
            {
                return {TokenKind::StringLiteral, end};
            }
        }

        // The longer of the two literals wins: '1' is an integer and '1e5' a floating-point number.
        size_t integer = integerEnd(text, start);
        size_t floatingPoint = floatingPointEnd(text, start);
        if (floatingPoint > integer)
        {
            return {TokenKind::FloatingPointLiteral, floatingPoint};
        }
        if (integer > start)
        {
            return {TokenKind::IntegerLiteral, integer};
        }

        if (size_t end = identifierEnd(text, start); end > start)
        {
            return {identifierKind(text.substr(start, end - start)), end};
        }
        for (const Spelling& mark : punctuationMarks)
        {
            if (text.substr(start).starts_with(mark.text))
            {
                return {mark.kind, start + mark.text.size()};
            }
        }

        // A character outside ASCII is invalid whole, with all the bytes of its UTF-8 encoding.
        return {TokenKind::Invalid, isNotAscii(c) ? skip(text, start, isNotAscii) : start + 1};
    }
}

bool
IceFmt::isDocComment(const Lexeme& lexeme)
{
    if (lexeme.kind == TokenKind::LineComment)
    {
        return lexeme.text.starts_with("///");
    }
    return lexeme.kind == TokenKind::BlockComment && lexeme.text.starts_with("/**") && lexeme.text[3] != '/' &&
           lexeme.text[3] != '*';
}

string_view
IceFmt::spelling(TokenKind kind)
{
    auto mark = ranges::find(punctuationMarks, kind, &Spelling::kind);
    assert(mark != punctuationMarks.end());
    return mark->text;
}

vector<Lexeme>
IceFmt::scan(string_view text)
{
    vector<Lexeme> lexemes;
    size_t offset = 0;
    int line = 1;
    int column = 0;
    while (offset < text.size())
    {
        auto [kind, end] = lexemeAt(text, offset);
        string_view lexeme = text.substr(offset, end - offset);
        if (kind)
        {
            lexemes.push_back({*kind, lexeme, line, column});
        }

        offset = end;
        for (char c : lexeme)
        {
            if (c == '\n')
            {
                line++;
                column = 0;
            }
            else
            {
                column++;
            }
        }
    }

    lexemes.push_back({TokenKind::EndOfFile, text.substr(offset), line, column});
    return lexemes;
}
