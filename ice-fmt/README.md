# ice-fmt

`ice-fmt` formats Slice files that use the original Slice syntax (`.ice` files). It has one style, with nothing to
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

`ice-fmt` reads a file without preprocessing it, so a macro that stands in for Slice syntax is a syntax error. A file
with a syntax error is reported as `file:line: message` and left unchanged.

The exit status is `1` when the command line is invalid, when a file can't be formatted, read, or written, and, with
`--check`, when a file would change.

## Style

- Indentation is 4 spaces, and lines are broken where they can be to fit in 120 columns.
- Braces go on their own line, except for an empty `{}` and a definition written on one line that still fits.
- A blank line separates a module, class, interface, exception, struct, or enum from the definitions around it, but
  not from its own forward declaration. Its metadata goes on the line above.
- An operation that doesn't fit moves its metadata to the line above, then its `throws` clause to its own line, then
  puts its parameters one per line. An `extends` clause moves to its own line first. Other lists that don't fit go one
  item per line.
- Other metadata stays on its line or the line above, as written, unless it doesn't fit there. Blank lines between
  definitions and members stay too, at most one in a row.
- Comments and directives keep their text and take the indentation of the code around them. A doc comment on a line
  with code moves above that line, where the Slice compiler reads it. The comments that end consecutive lines line
  up, one space after the longest code of those lines, unless that pushes one of them past 120 columns.
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
