# ice-fmt

`ice-fmt` formats Slice files that use the original Slice syntax (`.ice` files). It has one style, with nothing to
configure.

## Usage

```
ice-fmt [options] paths...
```

`ice-fmt` rewrites each file in place, and leaves a file that is already formatted untouched. A file given by name is
formatted whatever its name. A directory stands for the `.ice` files in it and in the directories below it; there, a
name that starts with a dot is skipped, and so is `node_modules`.

There is no list of files to skip. To leave files out, pass the files to format:

```
git ls-files -z '*.ice' ':!:test/errors' | xargs -0 ice-fmt --check
```

- `--check` writes nothing: it lists the files that formatting would change, and fails when there is one.
- `-h`, `--help` shows the usage.
- `-v`, `--version` shows the version.

A file with a syntax error is not formatted: `ice-fmt` reports the error as `file:line: message` and goes on with the
next file. It reads a file as written, without preprocessing it, so a file where a macro stands in for Slice syntax is
a syntax error too.

`ice-fmt` also refuses a file that it can't lay out again without changing what the Slice compiler reads:

- A comment or directive between a `(` and the name before it, which the compiler reads as one token.
- A comment or directive line that a backslash continues onto an empty line or the end of the file, or that has
  blanks after that backslash. The preprocessor joins such a line to the next one.

`ice-fmt` checks that the formatted text keeps every token, directive, and comment, and the definition each doc comment
documents, apart from the changes this README describes. A file that fails this check is left unchanged, even without
`--check`, and reported as `file: internal error: message`, which is a bug in `ice-fmt`.

The exit status is `1` when the command line is invalid, when a file has a syntax error, fails that check, or cannot be
read or written, and, with `--check`, when a file would change.

## Style

```slice
module Demo
{
    enum Color { Red, Green, Blue }

    exception PrinterException { string reason; }

    interface Printer
    {
        ["cpp:const"] idempotent string status() throws PrinterException;

        ["amd"]
        idempotent Ice::StringSeq listQueuedDocuments(string user, Color color, bool doubleSided, optional(1) int max);

        void cancel(string user, Ice::StringSeq documents, bool notifyOwner, out Ice::StringSeq cancelled)
            throws PrinterException;

        void print(
            string document,
            Color color,
            int copies,
            bool doubleSided,
            optional(1) string tray,
            out int pages,
            out optional(2) string warning)
            throws PrinterException;
    }
}
```

- Indentation is 4 spaces. A line is broken where it can be to fit in 120 columns; a comment that follows code doesn't
  count toward them.
- One space separates two tokens. None follows `(`, `<`, `[`, or `[[`, and none precedes `)`, `>`, `]`, `]]`, `,`,
  `;`, `(`, `<`, or `*`.
- Braces go on their own line. An empty body written as `{}` is the exception: it follows its definition on the same
  line.
- A blank line separates a module, class, interface, exception, struct, or enum from the definition before it and
  from the definition after it, but not from its own forward declaration. Its metadata goes on the line above it.
- A list goes on one line when it fits. When it doesn't:
  - An operation is broken in three steps, each one only when the previous ones are not enough: its metadata moves to
    the line above, its `throws` clause moves to its own line, then its parameters go one per line, after a line
    break that follows the `(`.
  - An `extends` clause moves to its own line. Its names, like those of a `throws` clause, go one per line if they
    still don't fit.
  - The types of a `sequence` or `dictionary` go one per line, after a line break that follows the `<`.
  - Metadata directives go one per line.
- Three choices are the author's:
  - The metadata of anything else, such as a sequence, a data member, or a parameter, sits on its line or on the
    line above. It moves to the line above when the two don't fit on one line.
  - A definition written on one line stays on one line when it fits.
  - A blank line between two definitions or two members stays.
- Metadata that doesn't lead, such as on the element type of a sequence or after `out`, stays on the line of what it
  applies to.
- Comments and preprocessor directives keep their text. One on its own line takes the indentation of the code around
  it, and the later lines of a block comment keep their indentation relative to its first line. A comment that follows
  code keeps its column when the code leaves room for it.
- A doc comment (`///` or `/** */`) on a line with code moves above that line. The Slice compiler reads it there: its
  preprocessor moves a comment above its line, so a doc comment documents the first definition of that line.
- No line ends with a space or a tab.
- At most one blank line separates two things, and none starts the file, follows a `{`, or precedes a `}`. The
  text of a block comment is the exception: its blank lines stay.
- No token is added or reordered. The only token removed is a `;` after a `}`, where Slice doesn't require one.
- A file keeps the line endings of its first line, and ends with one line break.

## Building

Building `ice-fmt` requires CMake 3.24 or later and a C++20 compiler.

```
cmake -S ice-fmt -B ice-fmt/build -DCMAKE_BUILD_TYPE=Release
cmake --build ice-fmt/build --config Release
```

The executable is `ice-fmt/build/ice-fmt`, or `ice-fmt\build\Release\ice-fmt.exe` with Visual Studio.

The `ice-fmt` workflow builds and tests the executable for macOS, Linux, and Windows, and uploads each one as an
artifact. It also checks the sources with the clang-format and clang-tidy of Homebrew's `llvm`, which is the latest
LLVM release. To run clang-tidy during a build, configure with `-DCMAKE_CXX_CLANG_TIDY=clang-tidy`.

## Testing

```
ctest --test-dir ice-fmt/build --build-config Release --output-on-failure
```

A test case is two files with the same name: `ice-fmt` must turn the one in `tests/unformatted` into the one in
`tests/formatted`, and leave the latter as it is. A new case also goes in the list of cases in `CMakeLists.txt`.
