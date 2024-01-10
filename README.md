# Slice for Visual Studio Code

This language extension adds support for highlighting Slice files within Visual Studio Code.
It supports highlighting the following file types:

- `.ice` files, which use the [original Slice](https://doc.zeroc.com/ice/latest/the-slice-language) syntax
- `.slice` files, which use the [latest Slice](https://docs.icerpc.dev/slice2) syntax

Additionally, this extension provides a Slice Language Server, for `.slice` files only.

## Configuration

The Slice Language Server can be configured using the following settings:

- `slice.languageServer.enabled`: A boolean indicating whether the Slice Language Server should be enabled. Defaults
  to `true`.
- `slice.configurations`: An array of configuration sets. Each configuration set represents independent slice
  compiler options alongside a corresponding compilation state. This allows you to have multiple files with different
  compilation states in the same workspace.
  - `paths`: An array containing the paths to the target Slice reference files. Also supports directory paths.
  - `includeBuiltInTypes`: A boolean indicating whether to include the built-in Slice reference files.

### Example

```json
{
    "slice.configurations": [
        {
            "paths": [
                "/path/to/slice/reference/files",
                "/path/to/other/slice/reference/files"
            ],
            "includeBuiltInTypes": true
        }
    ]
}
```
