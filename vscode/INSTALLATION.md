# Installation

## Installing from the Marketplace

This extension can be installed through the [Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=ZeroCInc.slice) by clicking "Install",
or from within VSCode by using Quick Open (`Ctrl + P`) to run the following command:

```shell
ext install "Slice"
```

## Installing from a Local Package

## Prerequisites

To install the extension's dependencies, run from the `vscode` directory:

```shell
npm install
```

The extension packages the Slice language server (written in Rust) from `vscode/server/target`.
To build it there, run from the repository root:
```shell
cargo build --release --target <TARGET> --manifest-path server/Cargo.toml --target-dir vscode/server/target
```
Where `TARGET` is one of the architectures where the extension will be run.
(See [Rust Supported Platforms](https://doc.rust-lang.org/beta/rustc/platform-support.html) for a full list of architectures).
To create an extension supported on multiple platforms, the server will need to be built multiple times,
each time targeting another platform architecture.

## Packaging

To build and package the extension, run from the `vscode` directory:

```shell
npx @vscode/vsce package
```

This generates a `.vsix` package file in the `vscode` directory.
This package can be installed by drag-and-dropping it into the "Extensions" panel of VSCode.
