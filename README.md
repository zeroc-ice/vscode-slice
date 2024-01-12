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
- `slice.configurations`: An array of configuration sets. Configuration sets are independently compiled, allowing each
  to define its own files and options for compilation.
  - **Each configuration set is an object with the following properties:**
  - `paths`: An array containing the paths to the target Slice reference files. Also supports directory paths.
  - `addWellKnownTypes`: A boolean indicating whether to include the
    [IceRpc well-known Slice files](https://github.com/icerpc/icerpc-slice/tree/main/WellKnownTypes) during compilation.

### Example

```json
{
    "slice.configurations": [
        {
            "paths": [
                "path/to/slice/directory",
                "path/to/other/slice/file.slice",
                "/absolute/path/to/directory"  
            ],
            "addWellKnownTypes": true
        }
    ]
}
```
