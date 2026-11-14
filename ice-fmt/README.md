# ice-fmt

`ice-fmt` formats Slice files that use the original Slice syntax (`.ice` files). It supports the Slice syntax of
Ice 3.8 and Ice 3.7, including local Slice. It has one style, with nothing to configure.

## Usage

```
ice-fmt [options] paths...
```

- `--check` writes nothing: it lists the files that formatting would change, and exits with status `1` if there is any.
- `-h`, `--help` shows the usage.
- `-v`, `--version` shows the version.

`ice-fmt` rewrites each file in place. Given a directory, it formats all the `.ice` files in that directory,
recursively. It skips the files and directories whose name starts with a dot, and `node_modules` directories, where
npm installs the Slice files of the Ice for JavaScript package.

To leave a file out, put the comment `// ice-fmt: ignore-file` in it.

`ice-fmt` has no option to exclude a directory: pass it the files to format instead. For example, this command checks
all the `.ice` files tracked by Git except the ones in `test/errors`:

```
git ls-files -z '*.ice' ':!:test/errors' | xargs -0 ice-fmt --check
```

To leave part of a file as written, put one of these comments on a line of its own:

- `// ice-fmt: ignore`, before a definition, leaves that definition as written.
- `// ice-fmt: off`, before a definition, leaves it and the definitions after it as written, up to the one that
  `// ice-fmt: on` precedes, or to the end of their module, class, or other body.

The text left as written takes the indentation of the code around it, and its lines keep their indentation relative to
the first one.

`ice-fmt` formats a file as written, without running the preprocessor. A file that is valid Slice only after
preprocessing has a syntax error for `ice-fmt`: for example, a file where a macro expands to metadata, or where an
`#ifdef` and its `#else` each start the same definition. `ice-fmt` reports a syntax error as `file:line: message` and
leaves the file unchanged.

## Style

`ice-fmt` indents with 4 spaces and breaks lines to fit in 120 columns where it can. Braces go on their own line,
except for an empty `{}` and a definition written on one line that still fits. `ice-fmt` keeps the text of comments and
preprocessor directives, and the blank lines between definitions, at most one in a row. The files in
[tests/formatted](tests/formatted) show the style in full.

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
