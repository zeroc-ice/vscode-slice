# slice-fmt

`slice-fmt` formats Slice files that use the original Slice syntax (`.ice` files). It has one style and no options.

## Usage

```
slice-fmt [options] paths...
```

`slice-fmt` rewrites each file in place, and leaves a file that is already formatted untouched. A directory stands for
the `.ice` files in it and in the directories below it. In a directory, a name that starts with a dot is skipped, and
so is `node_modules`.

- `--check` writes nothing: it lists the files that formatting would change, and fails when there is one.
- `-h`, `--help` shows the usage.
- `-v`, `--version` shows the version.

A file with a syntax error is not formatted: `slice-fmt` reports the error as `file:line: message` and goes on with the
next file. It reads a file as written, without preprocessing it, so a file where a macro stands in for Slice syntax is
a syntax error too.

The exit status is `1` when a file has a syntax error, cannot be read or written, or, with `--check`, would change.

## Style

```slice
module Demo
{
    enum Color { red, green, blue }

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

- Indentation is 4 spaces, and a line is at most 120 columns.
- Braces go on their own line. An empty body written as `{}` is the exception: it follows its definition on the same
  line.
- A list goes on one line when it fits. When it doesn't:
  - An operation is broken in three steps, each one only when the previous ones are not enough: its metadata moves to
    the line above, its `throws` clause moves to its own line, then its parameters go one per line, after a line
    break that follows the `(`.
  - An `extends` clause moves to its own line. Its names, like those of a `throws` clause, go one per line if they
    still don't fit.
  - Metadata directives go one per line.
- Three choices are the author's:
  - Metadata sits on the line of its declaration or on the line above, unless an operation doesn't fit on that line.
  - A definition written on one line stays on one line when it fits.
  - A blank line between two definitions or two members stays.
- Comments and preprocessor directives keep their text. One on its own line takes the indentation of the code around
  it, and a comment that follows code keeps its column.
- No line ends with a space or a tab.
- At most one blank line separates two things, and none starts the file, follows a `{`, or precedes a `}`. The
  text of a block comment is the exception: its blank lines stay.
- No token is added or reordered. The only token removed is a `;` after a `}`, where Slice doesn't require one.
- A file keeps its line endings, and ends with one line break.

## Building

Building `slice-fmt` requires CMake 3.24 or later and a C++20 compiler.

```
cmake -S slice-fmt -B slice-fmt/build -DCMAKE_BUILD_TYPE=Release
cmake --build slice-fmt/build --config Release
```

The executable is `slice-fmt/build/slice-fmt`, or `slice-fmt\build\Release\slice-fmt.exe` with Visual Studio.

The `slice-fmt` workflow builds the executable for macOS, Linux, and Windows, and uploads each one as an artifact.
