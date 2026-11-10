// Copyright (c) ZeroC, Inc.

#include "SyntaxParser.h"

#include <algorithm>
#include <array>
#include <string>

using namespace std;
using namespace SliceFormat;

namespace
{
    // The keywords that name a type. 'Object' is apart: it is the only one that '*' can follow.
    constexpr array<TokenKind, 9> typeKeywords{
        TokenKind::Bool,
        TokenKind::Byte,
        TokenKind::Short,
        TokenKind::Int,
        TokenKind::Long,
        TokenKind::Float,
        TokenKind::Double,
        TokenKind::String,
        TokenKind::Value};

    bool isTrivia(TokenKind kind)
    {
        return kind == TokenKind::LineComment || kind == TokenKind::BlockComment || kind == TokenKind::Directive;
    }

    // The preprocessor joins a line that ends with a backslash to the next one. Formatting removes the blanks that end
    // a line and manages the blank lines, so a comment or directive is refused where that would change which lines
    // are joined: blanks after a backslash, or an empty line after one.
    void checkContinuations(const Lexeme& lexeme)
    {
        int line = lexeme.line;
        string_view rest = lexeme.text;
        while (true)
        {
            size_t end = rest.find('\n');
            string_view current = rest.substr(0, end);
            if (current.ends_with('\r'))
            {
                current.remove_suffix(1);
            }

            size_t last = current.find_last_not_of(" \t");
            bool lastLine = end == string_view::npos;
            if (last == string_view::npos && lastLine && line > lexeme.line)
            {
                throw SyntaxError{line - 1, "line continued by a backslash onto an empty line"};
            }
            if (last != string_view::npos && current[last] == '\\' && last + 1 < current.size())
            {
                throw SyntaxError{line, "blanks after the backslash that ends the line"};
            }

            if (lastLine)
            {
                return;
            }
            rest.remove_prefix(end + 1);
            line++;
        }
    }

    // Groups the lexemes into tokens, and attaches each comment and directive to the token it sits next to.
    vector<Token> attachTrivia(const vector<Lexeme>& lexemes)
    {
        vector<Token> tokens;
        vector<Trivia> leading;

        // The line where the previous lexeme ends.
        int endLine = 0;

        for (auto lexeme = lexemes.begin(); lexeme != lexemes.end(); ++lexeme)
        {
            bool blankLineBefore = lexeme->line > endLine + 1;
            auto lineCount = static_cast<int>(ranges::count(lexeme->text, '\n'));

            if (isTrivia(lexeme->kind))
            {
                checkContinuations(*lexeme);

                // The lexeme of a directive starts with the blanks that indent it.
                size_t first = lexeme->text.find_first_not_of(" \t");
                string_view text = lexeme->text.substr(first);
                int column = lexeme->column + static_cast<int>(first);

                auto isToken = [](const Lexeme& other) { return !isTrivia(other.kind); };
                auto nextToken = find_if(lexeme + 1, lexemes.end(), isToken);
                bool tokenFollows =
                    nextToken->kind != TokenKind::EndOfFile && nextToken->line == lexeme->line + lineCount;

                // A comment that spans several lines never shares one with a token.
                if (!tokens.empty() && lexeme->line == tokens.back().line && lineCount == 0 && !tokenFollows)
                {
                    tokens.back().trailing.push_back({text, column, false, false});
                }
                else
                {
                    bool ownLine = lexeme->kind != TokenKind::BlockComment || lineCount > 0 || !tokenFollows;
                    leading.push_back({text, column, ownLine, blankLineBefore});
                }
            }
            else
            {
                bool firstOnLine = tokens.empty() || lexeme->line > tokens.back().line;
                tokens.push_back(
                    {lexeme->kind, lexeme->text, lexeme->line, firstOnLine, blankLineBefore, std::move(leading), {}});
                leading.clear();
            }

            endLine = lexeme->line + lineCount;
        }
        return tokens;
    }

    // A recursive descent parser for the Slice compilers' grammar, cpp/src/Slice/Grammar.y in zeroc-ice/ice, without
    // its error recovery rules.
    class Parser final
    {
    public:
        explicit Parser(vector<Token> tokens) : _tokens(std::move(tokens)) {}

        // file := definition* EndOfFile
        Node file()
        {
            Node root{NodeKind::File};
            while (!at(TokenKind::EndOfFile))
            {
                root.children.push_back(definition());
            }
            take(root);
            return root;
        }

    private:
        [[nodiscard]] const Token& current() const { return _tokens[_next]; }

        [[nodiscard]] bool at(TokenKind kind) const { return current().kind == kind; }

        [[nodiscard]] bool atScopedName() const { return at(TokenKind::Identifier) || at(TokenKind::ScopedIdentifier); }

        [[noreturn]] void fail(const string& expected) const
        {
            const Token& token = current();
            string found = token.kind == TokenKind::EndOfFile ? "the end of the file" : "'" + string{token.text} + "'";
            throw SyntaxError{token.line, "expected " + expected + ", found " + found};
        }

        void take(Node& parent)
        {
            parent.children.emplace_back(std::move(_tokens[_next]));
            _next++;
        }

        bool accept(Node& parent, TokenKind kind)
        {
            if (!at(kind))
            {
                return false;
            }
            take(parent);
            return true;
        }

        void expect(Node& parent, TokenKind kind)
        {
            if (!at(kind))
            {
                fail("'" + string{spelling(kind)} + "'");
            }
            take(parent);
        }

        void name(Node& parent)
        {
            if (!at(TokenKind::Identifier))
            {
                fail("a name");
            }
            take(parent);
        }

        void scopedName(Node& parent)
        {
            if (!atScopedName())
            {
                fail("a name");
            }
            take(parent);
        }

        // type := typeKeyword | 'Object' '*'? | scopedName '*'?
        void type(Node& parent)
        {
            bool canBeProxy = atScopedName() || at(TokenKind::Object);
            if (!canBeProxy && ranges::find(typeKeywords, current().kind) == typeKeywords.end())
            {
                fail("a type");
            }
            take(parent);
            if (canBeProxy)
            {
                accept(parent, TokenKind::Asterisk);
            }
        }

        // optionalType := ('optional' '(' integerConstant ')')? type
        void optionalType(Node& parent)
        {
            if (accept(parent, TokenKind::Optional))
            {
                expect(parent, TokenKind::LeftParenthesis);
                integerConstant(parent);
                expect(parent, TokenKind::RightParenthesis);
            }
            type(parent);
        }

        // integerConstant := IntegerLiteral | scopedName
        void integerConstant(Node& parent)
        {
            if (!at(TokenKind::IntegerLiteral) && !atScopedName())
            {
                fail("an integer");
            }
            take(parent);
        }

        // constInitializer := IntegerLiteral | FloatingPointLiteral | StringLiteral | 'true' | 'false' | scopedName
        void constInitializer(Node& parent)
        {
            bool literal = at(TokenKind::IntegerLiteral) || at(TokenKind::FloatingPointLiteral) ||
                           at(TokenKind::StringLiteral) || at(TokenKind::True) || at(TokenKind::False);
            if (!literal && !atScopedName())
            {
                fail("a value");
            }
            take(parent);
        }

        // metadata := '[' (StringLiteral+ (',' StringLiteral+)*)? ']'
        // fileMetadata := '[[' (StringLiteral+ (',' StringLiteral+)*)? ']]'
        Node metadata(TokenKind open, TokenKind close)
        {
            Node node{NodeKind::Metadata};
            expect(node, open);
            if (!at(close))
            {
                do
                {
                    // Adjacent string literals make a single directive.
                    do
                    {
                        if (!at(TokenKind::StringLiteral))
                        {
                            fail("a string");
                        }
                        take(node);
                    } while (at(TokenKind::StringLiteral));
                } while (accept(node, TokenKind::Comma));
            }
            expect(node, close);
            return node;
        }

        void localMetadata(Node& parent)
        {
            while (at(TokenKind::LeftBracket))
            {
                parent.children.push_back(metadata(TokenKind::LeftBracket, TokenKind::RightBracket));
            }
        }

        // definition := fileMetadata
        //             | metadata* (moduleDefinition | typeDefinition | enum | sequence | dictionary | const)
        Node definition()
        {
            if (at(TokenKind::DoubleLeftBracket))
            {
                return metadata(TokenKind::DoubleLeftBracket, TokenKind::DoubleRightBracket);
            }

            Node declaration{NodeKind::Declaration};
            localMetadata(declaration);
            if (at(TokenKind::Module))
            {
                moduleDefinition(declaration);
            }
            else if (
                at(TokenKind::Class) || at(TokenKind::Interface) || at(TokenKind::Exception) || at(TokenKind::Struct))
            {
                typeDefinition(declaration);
            }
            else if (at(TokenKind::Enum))
            {
                enumeration(declaration);
            }
            else if (at(TokenKind::Sequence))
            {
                sequence(declaration);
            }
            else if (at(TokenKind::Dictionary))
            {
                dictionary(declaration);
            }
            else if (at(TokenKind::Const))
            {
                constant(declaration);
            }
            else
            {
                fail("a definition");
            }
            return declaration;
        }

        // moduleDefinition := 'module' scopedName '{' definition* '}' ';'?
        void moduleDefinition(Node& declaration)
        {
            take(declaration);
            scopedName(declaration);

            Node body{NodeKind::Body};
            expect(body, TokenKind::LeftBrace);
            while (!at(TokenKind::RightBrace))
            {
                body.children.push_back(definition());
            }
            take(body);
            declaration.children.push_back(std::move(body));
            accept(declaration, TokenKind::Semicolon);
        }

        // typeDefinition := ('class' | 'interface') name ';'
        //                 | 'class' name ('(' integerConstant ')')? (extends scopedName)? dataMembers ';'?
        //                 | 'interface' name (extends base (',' base)*)? operations ';'?
        //                 | 'exception' name (extends scopedName)? dataMembers ';'?
        //                 | 'struct' name dataMembers ';'?
        // extends := 'extends' | ':'
        // base := scopedName | 'Object' | 'Value'
        // dataMembers := '{' dataMember* '}'
        // operations := '{' operation* '}'
        void typeDefinition(Node& declaration)
        {
            TokenKind keyword = current().kind;
            take(declaration);
            name(declaration);

            // A forward declaration.
            if ((keyword == TokenKind::Class || keyword == TokenKind::Interface) &&
                accept(declaration, TokenKind::Semicolon))
            {
                return;
            }

            if (keyword == TokenKind::Class && accept(declaration, TokenKind::LeftParenthesis))
            {
                integerConstant(declaration);
                expect(declaration, TokenKind::RightParenthesis);
            }
            if (keyword != TokenKind::Struct && (at(TokenKind::Extends) || at(TokenKind::Colon)))
            {
                Node clause{NodeKind::Clause};
                take(clause);
                if (keyword == TokenKind::Interface)
                {
                    do
                    {
                        if (!atScopedName() && !at(TokenKind::Object) && !at(TokenKind::Value))
                        {
                            fail("a name");
                        }
                        take(clause);
                    } while (accept(clause, TokenKind::Comma));
                }
                else
                {
                    scopedName(clause);
                }
                declaration.children.push_back(std::move(clause));
            }

            Node body{NodeKind::Body};
            expect(body, TokenKind::LeftBrace);
            while (!at(TokenKind::RightBrace))
            {
                body.children.push_back(keyword == TokenKind::Interface ? operation() : dataMember());
            }
            take(body);
            declaration.children.push_back(std::move(body));
            accept(declaration, TokenKind::Semicolon);
        }

        // dataMember := metadata* optionalType name ('=' constInitializer)? ';'
        Node dataMember()
        {
            Node member{NodeKind::Declaration};
            localMetadata(member);
            optionalType(member);
            name(member);
            if (accept(member, TokenKind::Equals))
            {
                constInitializer(member);
            }
            expect(member, TokenKind::Semicolon);
            return member;
        }

        // operation := metadata* 'idempotent'? ('void' | optionalType) name parameters throws? ';'
        // parameters := '(' (parameter (',' parameter)*)? ')'
        // throws := 'throws' scopedName (',' scopedName)*
        Node operation()
        {
            Node member{NodeKind::Declaration};
            localMetadata(member);
            accept(member, TokenKind::Idempotent);
            if (!accept(member, TokenKind::Void))
            {
                optionalType(member);
            }
            name(member);

            Node parameters{NodeKind::Parameters};
            expect(parameters, TokenKind::LeftParenthesis);
            if (!at(TokenKind::RightParenthesis))
            {
                do
                {
                    parameters.children.push_back(parameter());
                } while (accept(parameters, TokenKind::Comma));
            }
            expect(parameters, TokenKind::RightParenthesis);
            member.children.push_back(std::move(parameters));

            if (at(TokenKind::Throws))
            {
                Node clause{NodeKind::Clause};
                take(clause);
                do
                {
                    scopedName(clause);
                } while (accept(clause, TokenKind::Comma));
                member.children.push_back(std::move(clause));
            }
            expect(member, TokenKind::Semicolon);
            return member;
        }

        // parameter := (metadata | 'out')* optionalType name
        Node parameter()
        {
            Node declaration{NodeKind::Declaration};
            do
            {
                localMetadata(declaration);
            } while (accept(declaration, TokenKind::Out));
            optionalType(declaration);
            name(declaration);
            return declaration;
        }

        // enum := 'enum' name '{' (enumerator (',' enumerator)* ','?)? '}' ';'?
        // enumerator := metadata* name ('=' integerConstant)?
        void enumeration(Node& declaration)
        {
            take(declaration);
            name(declaration);

            Node body{NodeKind::Body};
            expect(body, TokenKind::LeftBrace);
            while (!at(TokenKind::RightBrace))
            {
                Node enumerator{NodeKind::Declaration};
                localMetadata(enumerator);
                name(enumerator);
                if (accept(enumerator, TokenKind::Equals))
                {
                    integerConstant(enumerator);
                }
                body.children.push_back(std::move(enumerator));

                if (!at(TokenKind::RightBrace))
                {
                    expect(body, TokenKind::Comma);
                }
            }
            take(body);
            declaration.children.push_back(std::move(body));
            accept(declaration, TokenKind::Semicolon);
        }

        // sequence := 'sequence' '<' metadata* type '>' name ';'
        void sequence(Node& declaration)
        {
            take(declaration);
            expect(declaration, TokenKind::LeftAngle);
            localMetadata(declaration);
            type(declaration);
            expect(declaration, TokenKind::RightAngle);
            name(declaration);
            expect(declaration, TokenKind::Semicolon);
        }

        // dictionary := 'dictionary' '<' metadata* type ',' metadata* type '>' name ';'
        void dictionary(Node& declaration)
        {
            take(declaration);
            expect(declaration, TokenKind::LeftAngle);
            localMetadata(declaration);
            type(declaration);
            expect(declaration, TokenKind::Comma);
            localMetadata(declaration);
            type(declaration);
            expect(declaration, TokenKind::RightAngle);
            name(declaration);
            expect(declaration, TokenKind::Semicolon);
        }

        // const := 'const' metadata* type name '=' constInitializer ';'
        void constant(Node& declaration)
        {
            take(declaration);
            localMetadata(declaration);
            type(declaration);
            name(declaration);
            expect(declaration, TokenKind::Equals);
            constInitializer(declaration);
            expect(declaration, TokenKind::Semicolon);
        }

        vector<Token> _tokens;
        size_t _next = 0;
    };
}

Node
SliceFormat::parse(string_view text)
{
    return Parser{attachTrivia(scan(text))}.file();
}
