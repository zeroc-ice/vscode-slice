# Installing from the Marketplace

This extension can be installed through the [Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=ZeroCInc.slice) by clicking "Install",
or from within VSCode by using Quick Open (`Ctrl + P`) to run the following command:

```
ext install "Slice"
```

# Installing from a Local Package

## Prerequisites

To build and package the extension, you must have [`vsce`](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#vsce) installed:

```
npm install -g @vscode/vsce
```

## Packaging

To build and package the extension, run:

```
vsce package
```

This generates a `.vsix` package file in the root directory of the project.
This package can be installed by drag-and-dropping it into the "Extensions" panel of VSCode.
