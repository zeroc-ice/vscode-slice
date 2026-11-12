// Copyright (c) ZeroC, Inc.

#include "SyntaxParser.h"
#include "SyntaxError.h"
#include "SyntaxScanner.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

using namespace std;
using namespace IceFmt;

namespace
{
    // The builtin types. 'Object' is the only one that '*' can follow.
    constexpr array builtinTypes{
        TokenKind::Bool,
        TokenKind::Byte,
        TokenKind::Short,
        TokenKind::Int,
        TokenKind::Long,
        TokenKind::Float,
        TokenKind::Double,
        TokenKind::String,
        TokenKind::Object,
        TokenKind::Value};

    bool isTrivia(TokenKind kind)
    {
        return kind == TokenKind::LineComment || kind == TokenKind::BlockComment || kind == TokenKind::Directive;
    }

    // The preprocessor joins a line that ends with a backslash to the next one. Formatting manages the blank lines and
    // ends the file with a line break, so a comment or directive is refused where that would change which lines are
    // joined: an empty line or the end of the file after a backslash.
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

            if (lastLine)
            {
                // Only the end of the file stops a line that a backslash continues.
                if (current.ends_with('\\'))
                {
                    throw SyntaxError{line, "line continued by a backslash at the end of the file"};
                }
                return;
            }
            rest.remove_prefix(end + 1);
            line++;
        }
    }

    // Where a comment or directive goes, relative to the tokens around it.
    enum class Placement
    {
        // On lines of its own, before the next token.
        OwnLine,

        // On the line of the next token, before it.
        BeforeToken,

        // At the end of the line of the previous token.
        AfterToken,

        // Above its line, as the preprocessor of the Slice compiler counts lines: a block comment joins the lines it
        // spans. The preprocessor moves a comment above its line, so a doc comment after code documents the first
        // definition of that line. Attached to the first token of that line, it keeps documenting that definition when
        // the line is laid out again.
        AboveLine,
    };

    // Returns where a comment or directive goes. 'tokens' are the tokens before it, 'joinedLine' is the line where its
    // line starts for the preprocessor, which joins the lines a block comment spans, and 'tokenFollows' is whether a
    // token follows it on its last line.
    Placement placement(const Lexeme& trivia, const vector<Token>& tokens, int joinedLine, bool tokenFollows)
    {
        bool docComment = isDocComment(trivia);
        if (docComment && !tokens.empty() && tokens.back().line >= joinedLine)
        {
            return Placement::AboveLine;
        }

        // A comment that spans several lines never shares one with a token. A doc comment before code on its line gets
        // the line to itself, which changes nothing for the compiler.
        bool oneLine = trivia.text.find('\n') == string_view::npos;
        if (oneLine && !tokenFollows && !tokens.empty() && tokens.back().line == trivia.line)
        {
            return Placement::AfterToken;
        }
        if (oneLine && tokenFollows && trivia.kind == TokenKind::BlockComment && !docComment)
        {
            return Placement::BeforeToken;
        }
        return Placement::OwnLine;
    }

    // Puts a comment above the line that starts at 'joinedLine', between what leads the first token of that line and
    // that token. A comment that led that token on its line no longer shares the line with it.
    void moveAboveLine(vector<Token>& tokens, int joinedLine, Trivia comment)
    {
        auto first = ranges::lower_bound(tokens, joinedLine, {}, &Token::line);
        for (Trivia& trivia : first->leading)
        {
            trivia.ownLine = true;
        }
        comment.blankLineBefore = first->blankLineBefore;
        first->leading.push_back(comment);
        first->blankLineBefore = false;
    }

    // Refuses a comment or directive between a '(' and the name before it: the compiler reads the two as one token, and
    // laying out the lines around the comment could leave it between them.
    void checkParentheses(const vector<Token>& tokens)
    {
        for (size_t i = 1; i < tokens.size(); ++i)
        {
            if (tokens[i].kind == TokenKind::LeftParenthesis &&
                (!tokens[i].leading.empty() || !tokens[i - 1].trailing.empty()))
            {
                throw SyntaxError{tokens[i].line, "comment or directive before '('"};
            }
        }
    }

    // Groups the lexemes into tokens, and attaches each comment and directive to the token it sits next to.
    vector<Token> attachTrivia(const vector<Lexeme>& lexemes)
    {
        vector<Token> tokens;
        vector<Trivia> leading;

        // The line where the previous lexeme ends.
        int endLine = 0;

        // The line where the preprocessor's current line starts.
        int joinedLine = 0;

        for (auto lexeme = lexemes.begin(); lexeme != lexemes.end(); ++lexeme)
        {
            bool blankLineBefore = lexeme->line > endLine + 1;
            if (lexeme->line > endLine)
            {
                joinedLine = lexeme->line;
            }
            int lastLine = lexeme->line + static_cast<int>(ranges::count(lexeme->text, '\n'));
            endLine = lastLine;

            if (!isTrivia(lexeme->kind))
            {
                bool firstOnLine = tokens.empty() || lexeme->line > tokens.back().line;
                tokens.push_back(
                    {.kind = lexeme->kind,
                     .text = lexeme->text,
                     .line = lexeme->line,
                     .startsLine = firstOnLine,
                     .blankLineBefore = blankLineBefore,
                     .leading = std::move(leading),
                     .trailing = {}});
                leading.clear();
                continue;
            }

            checkContinuations(*lexeme);
            auto nextToken = ranges::find_if_not(lexeme + 1, lexemes.end(), isTrivia, &Lexeme::kind);
            bool tokenFollows = nextToken->kind != TokenKind::EndOfFile && nextToken->line == lastLine;
            Trivia trivia{
                .text = lexeme->text,
                .column = lexeme->column,
                .ownLine = true,
                .blankLineBefore = blankLineBefore};
            switch (placement(*lexeme, tokens, joinedLine, tokenFollows))
            {
                case Placement::OwnLine:
                    leading.push_back(trivia);
                    break;
                case Placement::BeforeToken:
                    trivia.ownLine = false;
                    leading.push_back(trivia);
                    break;
                case Placement::AfterToken:
                    trivia.ownLine = false;
                    trivia.blankLineBefore = false;
                    tokens.back().trailing.push_back(trivia);
                    break;
                case Placement::AboveLine:
                    moveAboveLine(tokens, joinedLine, trivia);
                    break;
            }
        }

        checkParentheses(tokens);
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

        // type := builtin | 'Object' '*' | scopedName '*'?
        void type(Node& parent)
        {
            bool canBeProxy = atScopedName() || at(TokenKind::Object);
            if (!atScopedName() && ranges::find(builtinTypes, current().kind) == builtinTypes.end())
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
                fail("an integer constant");
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

        // metadata := '[' (directive (',' directive)*)? ']'
        // fileMetadata := '[[' (directive (',' directive)*)? ']]'
        Node metadata(TokenKind open, TokenKind close)
        {
            Node node{NodeKind::Metadata};
            expect(node, open);
            if (!at(close))
            {
                do
                {
                    directive(node);
                } while (accept(node, TokenKind::Comma));
            }
            expect(node, close);
            return node;
        }

        // directive := StringLiteral+
        void directive(Node& parent)
        {
            // Adjacent string literals make a single directive.
            do
            {
                if (!at(TokenKind::StringLiteral))
                {
                    fail("a string literal");
                }
                take(parent);
            } while (at(TokenKind::StringLiteral));
        }

        void localMetadata(Node& parent)
        {
            while (at(TokenKind::LeftBracket))
            {
                parent.children.push_back(metadata(TokenKind::LeftBracket, TokenKind::RightBracket));
            }
        }

        // definition := fileMetadata
        //             | metadata* (moduleDefinition | classDefinition | interfaceDefinition | exceptionDefinition
        //                          | structDefinition | enum | sequence | dictionary | const)
        Node definition()
        {
            if (at(TokenKind::DoubleLeftBracket))
            {
                return metadata(TokenKind::DoubleLeftBracket, TokenKind::DoubleRightBracket);
            }

            Node contained{NodeKind::Contained};
            localMetadata(contained);
            switch (current().kind)
            {
                case TokenKind::Module:
                    moduleDefinition(contained);
                    break;
                case TokenKind::Class:
                    classDefinition(contained);
                    break;
                case TokenKind::Interface:
                    interfaceDefinition(contained);
                    break;
                case TokenKind::Exception:
                    exceptionDefinition(contained);
                    break;
                case TokenKind::Struct:
                    structDefinition(contained);
                    break;
                case TokenKind::Enum:
                    enumeration(contained);
                    break;
                case TokenKind::Sequence:
                    sequence(contained);
                    break;
                case TokenKind::Dictionary:
                    dictionary(contained);
                    break;
                case TokenKind::Const:
                    constant(contained);
                    break;
                default:
                    fail("a definition");
            }
            return contained;
        }

        // body := '{' member* '}' ';'?
        // Each member is the node that 'member' returns.
        void body(Node& contained, const auto& member)
        {
            Node node{NodeKind::Body};
            expect(node, TokenKind::LeftBrace);
            while (!at(TokenKind::RightBrace))
            {
                node.children.push_back(member());
            }
            take(node);
            contained.children.push_back(std::move(node));
            accept(contained, TokenKind::Semicolon);
        }

        // moduleDefinition := 'module' scopedName '{' definition* '}' ';'?
        void moduleDefinition(Node& contained)
        {
            take(contained);
            scopedName(contained);
            body(contained, [this] { return definition(); });
        }

        // classDefinition := 'class' name ';'
        //                  | 'class' name compactId? extends? '{' dataMember* '}' ';'?
        // compactId := '(' integerConstant ')'
        void classDefinition(Node& contained)
        {
            take(contained);
            name(contained);
            if (accept(contained, TokenKind::Semicolon))
            {
                // A forward declaration.
                return;
            }

            if (accept(contained, TokenKind::LeftParenthesis))
            {
                integerConstant(contained);
                expect(contained, TokenKind::RightParenthesis);
            }
            extends(contained);
            body(contained, [this] { return dataMember(); });
        }

        // interfaceDefinition := 'interface' name ';'
        //                      | 'interface' name interfaceExtends? '{' operation* '}' ';'?
        void interfaceDefinition(Node& contained)
        {
            take(contained);
            name(contained);
            if (accept(contained, TokenKind::Semicolon))
            {
                // A forward declaration.
                return;
            }

            interfaceExtends(contained);
            body(contained, [this] { return operation(); });
        }

        // exceptionDefinition := 'exception' name extends? '{' dataMember* '}' ';'?
        void exceptionDefinition(Node& contained)
        {
            take(contained);
            name(contained);
            extends(contained);
            body(contained, [this] { return dataMember(); });
        }

        // structDefinition := 'struct' name '{' dataMember* '}' ';'?
        void structDefinition(Node& contained)
        {
            take(contained);
            name(contained);
            body(contained, [this] { return dataMember(); });
        }

        // extends := ('extends' | ':') scopedName
        void extends(Node& contained)
        {
            if (!at(TokenKind::Extends) && !at(TokenKind::Colon))
            {
                return;
            }

            Node clause{NodeKind::Clause};
            take(clause);
            scopedName(clause);
            contained.children.push_back(std::move(clause));
        }

        // interfaceExtends := ('extends' | ':') interfaceList
        // interfaceList := (scopedName | 'Object' | 'Value') (',' (scopedName | 'Object' | 'Value'))*
        void interfaceExtends(Node& contained)
        {
            if (!at(TokenKind::Extends) && !at(TokenKind::Colon))
            {
                return;
            }

            Node clause{NodeKind::Clause};
            take(clause);
            do
            {
                if (!atScopedName() && !at(TokenKind::Object) && !at(TokenKind::Value))
                {
                    fail("a name");
                }
                take(clause);
            } while (accept(clause, TokenKind::Comma));
            contained.children.push_back(std::move(clause));
        }

        // dataMember := metadata* optionalType name ('=' constInitializer)? ';'
        Node dataMember()
        {
            Node member{NodeKind::Contained};
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
        Node operation()
        {
            Node member{NodeKind::Operation};
            localMetadata(member);
            accept(member, TokenKind::Idempotent);
            if (!accept(member, TokenKind::Void))
            {
                optionalType(member);
            }
            name(member);
            member.children.push_back(parameters());
            throws(member);
            expect(member, TokenKind::Semicolon);
            return member;
        }

        // parameters := '(' (parameter (',' parameter)*)? ')'
        Node parameters()
        {
            Node node{NodeKind::Parameters};
            expect(node, TokenKind::LeftParenthesis);
            if (!at(TokenKind::RightParenthesis))
            {
                do
                {
                    node.children.push_back(parameter());
                } while (accept(node, TokenKind::Comma));
            }
            expect(node, TokenKind::RightParenthesis);
            return node;
        }

        // throws := 'throws' scopedName (',' scopedName)*
        void throws(Node& member)
        {
            if (!at(TokenKind::Throws))
            {
                return;
            }

            Node clause{NodeKind::Clause};
            take(clause);
            do
            {
                scopedName(clause);
            } while (accept(clause, TokenKind::Comma));
            member.children.push_back(std::move(clause));
        }

        // parameter := (metadata | 'out')* optionalType name
        Node parameter()
        {
            Node contained{NodeKind::Contained};
            do
            {
                localMetadata(contained);
            } while (accept(contained, TokenKind::Out));
            optionalType(contained);
            name(contained);
            return contained;
        }

        // enum := 'enum' name '{' (enumerator (',' enumerator)* ','?)? '}' ';'?
        void enumeration(Node& contained)
        {
            take(contained);
            name(contained);

            // Unlike the other bodies, commas separate the members.
            Node members{NodeKind::Body};
            expect(members, TokenKind::LeftBrace);
            while (!at(TokenKind::RightBrace))
            {
                members.children.push_back(enumerator());
                if (!at(TokenKind::RightBrace))
                {
                    expect(members, TokenKind::Comma);
                }
            }
            take(members);
            contained.children.push_back(std::move(members));
            accept(contained, TokenKind::Semicolon);
        }

        // enumerator := metadata* name ('=' integerConstant)?
        Node enumerator()
        {
            Node node{NodeKind::Contained};
            localMetadata(node);
            name(node);
            if (accept(node, TokenKind::Equals))
            {
                integerConstant(node);
            }
            return node;
        }

        // sequence := 'sequence' '<' metadata* type '>' name ';'
        void sequence(Node& contained)
        {
            take(contained);
            Node types{NodeKind::Types};
            expect(types, TokenKind::LeftAngle);
            localMetadata(types);
            type(types);
            expect(types, TokenKind::RightAngle);
            contained.children.push_back(std::move(types));
            name(contained);
            expect(contained, TokenKind::Semicolon);
        }

        // dictionary := 'dictionary' '<' metadata* type ',' metadata* type '>' name ';'
        void dictionary(Node& contained)
        {
            take(contained);
            Node types{NodeKind::Types};
            expect(types, TokenKind::LeftAngle);
            localMetadata(types);
            type(types);
            expect(types, TokenKind::Comma);
            localMetadata(types);
            type(types);
            expect(types, TokenKind::RightAngle);
            contained.children.push_back(std::move(types));
            name(contained);
            expect(contained, TokenKind::Semicolon);
        }

        // const := 'const' metadata* type name '=' constInitializer ';'
        void constant(Node& contained)
        {
            take(contained);
            localMetadata(contained);
            type(contained);
            name(contained);
            expect(contained, TokenKind::Equals);
            constInitializer(contained);
            expect(contained, TokenKind::Semicolon);
        }

        vector<Token> _tokens;
        size_t _next = 0;
    };
}

Node
IceFmt::parse(string_view text)
{
    return Parser{attachTrivia(scan(text))}.file();
}
