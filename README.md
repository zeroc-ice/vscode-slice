# Slice Extension for Visual Studio Code

This simple [language extension](https://code.visualstudio.com/docs/extensionAPI/language-support) adds support for [Slice](https://doc.zeroc.com/ice/latest/the-slice-language) syntax highlighting to Visual Studio Code and other environments, such as GitHub and the Visual Studio IDE.

# Manually Installing for Visual Studio Code

To manually install this extension with Visual Studio, first navigate to the `.vscode/extensions` folder in your home directory (If this folder doesn't already exist, you'll have to create it). Then create a new folder titled `vscode-slice` and copy `package.json` and the `syntaxes` folder from this repository into it.

# Manually Installing for Visual Studio

To manually install this extension with Visual Studio, first navigate to the `.vs/extensions` folder in your home directory (If this folder doesn't already exist, you'll have to create it). Then create a new folder titled `vscode-slice` and copy the `syntaxes` folder from this repository into it.

# .ice Files

Slice definitions are always stored in files with the `.ice` extension.

# TextMate Grammar for Slice

The [TextMate grammar for Slice](syntaxes/slice.tmLanguage.json) file describes the grammar and is the most important file in this repository.
