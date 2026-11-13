# ice-fmt

`ice-fmt` formats Slice files that use the original Slice syntax (`.ice` files), as Ice 3.8 reads it or as Ice 3.7 did,
with its `local` definitions, `implements` clauses, and operations in classes. It has one style, with nothing to
configure.

## Usage

```
ice-fmt [options] paths...
```

`ice-fmt` rewrites each file in place. A directory stands for the `.ice` files below it, skipping `node_modules` and any
name that starts with a dot. To leave a file out, put the comment `// ice-fmt: ignore-file` in it, or pass only the
files to format:

```
git ls-files -z '*.ice' ':!:test/errors' | xargs -0 ice-fmt --check
```

- `--check` writes nothing: it lists the files that formatting would change.
- `-h`, `--help` shows the usage.
- `-v`, `--version` shows the version.

To leave part of a file as written, put one of these comments on a line of its own:

- `// ice-fmt: ignore`, before a definition or member, leaves that definition or member as written.
- `// ice-fmt: off`, before a definition or member, leaves it and the definitions and members after it as written, up to
  the one that `// ice-fmt: on` precedes, or to the end of their module, class, or other body.

The text left as written takes the indentation of the code around it, and its lines keep their indentation relative to
the first one.

`ice-fmt` reads a file without preprocessing it, so a macro that stands in for Slice syntax is a syntax error. A file
with a syntax error is reported as `file:line: message` and left unchanged.

The exit status is `1` when the command line is invalid, when a file can't be formatted, read, or written, and, with
`--check`, when a file would change.

## Style

- Indentation is 4 spaces, and lines are broken where they can be to fit in 120 columns.
- Braces go on their own line, except for an empty `{}` and a definition written on one line that still fits.
- A blank line separates a module, class, interface, exception, struct, or enum from the definitions around it, but
  not from its own forward declaration. Other blank lines stay, at most one in a row.
- An operation goes on one line when it fits, whatever line breaks the author put in it. When it doesn't fit, its
  `throws` clause moves to its own line, then its parameters go one per line.
- An `extends` clause that doesn't fit moves to its own line, then its names go one per line. Any other list that
  doesn't fit goes one item per line.
- Metadata:
  - The metadata of a module, class, interface, exception, struct, enum, or operation goes on the line above it. So
    does the metadata of a forward declaration.
  - The metadata of a sequence, dictionary, constant, data member, enumerator, or parameter stays on its line or on
    the line above, as the author wrote it. It moves to the line above when the two don't fit on one line.
  - Several metadata lists go one per line, and what they lead starts the next line. In a parameter list, this puts
    each parameter on its own line.
- Comments and directives keep their text and take the indentation of the code around them.
- A doc comment on a line with code moves above that line, where the Slice compiler reads it.
- The comments that end consecutive lines start in one column, one space after the longest code of those lines.
- No line ends with a space or a tab, and the file ends with one line break.

## Building

Building `ice-fmt` requires CMake 3.24 or later and a C++20 compiler.

```
cmake -S ice-fmt -B ice-fmt/build -DCMAKE_BUILD_TYPE=Release
cmake --build ice-fmt/build --config Release
```

The executable is `ice-fmt/build/ice-fmt`, or `ice-fmt\build\Release\ice-fmt.exe` with Visual Studio.

## Testing

```
ctest --test-dir ice-fmt/build --build-config Release --output-on-failure
```

A test case is two files with the same name: `ice-fmt` must turn the one in `tests/unformatted` into the one in
`tests/formatted`, and leave the latter as it is. A new case also goes in the list of cases in `CMakeLists.txt`.
