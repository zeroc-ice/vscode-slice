# Slice Extension for Visual Studio Code

This language extension adds support for highlighting Slice files within Visual Studio Code.  
It supports `.ice` files (which use the [original Slice](https://doc.zeroc.com/ice/latest/the-slice-language) syntax)
and `.slice` files (which use the [latest Slice](https://docs.icerpc.dev/slice2) syntax).

## Installation

The extension can be installed through the [Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=ZeroCInc.slice) by clicking "Install",  
or from within VSCode by using Quick Open (`Ctrl + P`) to run the following command:
```
ext install "Slice Highlighter"
```

## Development

The extension can be tested locally through VSCode by selecting "Extension" from the "Start Debugging" section.  
This starts a new instance of VSCode that has the extension installed.

To create a test package, you must have [`vsce`](https://code.visualstudio.com/api/working-with-extensions/publishing-extension#vsce) installed:
```
npm install -g @vscode/vsce
```

Then to package the extension, run:
```
vsce package
```

This creates a `.vsix` file that can be installed by drag-and-dropping it into the "Extensions panel of VSCode.
