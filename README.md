# Slice Tools

Editor tools for the Slice interface definition language: `.ice` files for [Ice] and `.slice` files for [IceRPC].

| Directory                        | Description                                                                                  |
|----------------------------------|----------------------------------------------------------------------------------------------|
| [server](server)                 | The Slice language server for `.slice` files, written in Rust.                               |
| [editors/vscode](editors/vscode) | The [Slice extension for Visual Studio Code][marketplace], which bundles the language server. |

GitHub highlights `.ice` files with the grammar in [editors/vscode/syntaxes](editors/vscode/syntaxes), through [Linguist].

## Building

[editors/vscode/INSTALLATION.md](editors/vscode/INSTALLATION.md) describes how to build the language server and package the extension.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Slice Tools is licensed under the [BSD 3-Clause License](LICENSE).

[Ice]: https://github.com/zeroc-ice/ice
[IceRPC]: https://github.com/icerpc
[Linguist]: https://github.com/github-linguist/linguist
[marketplace]: https://marketplace.visualstudio.com/items?itemName=ZeroCInc.slice
