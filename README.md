# Slice for Visual Studio Code

This extension provides support for the Slice Interface Definition Language (IDL).

## Features

- Syntax highlighting
- Syntax validation
- Error detection & reporting
- 'Go to Definition' jumping

### Syntax Highlighting and Validation

This extension supports syntax highlighting and validation for the following Slice file types:

- `.ice` files, which use the [original Slice](https://doc.zeroc.com/ice/latest/the-slice-language) syntax
- `.slice` files, which use the [latest Slice](https://docs.icerpc.dev/slice2) syntax

### Error Detection and Reporting

Error checking is triggered every time a Slice file is saved or opened,
and is only available for `.slice` files.

## Configuration

The Slice language server that ships with this extension can be configured with the following settings:

- `slice.languageServer.enabled`: A boolean indicating whether the server should be enabled. Defaults to `true`.

- `slice.configurations`: An array of configuration sets (independent groups of Slice files). This allows multiple Slice projects to exist within a single repository. Each configuration set supports the following settings:

    - `paths`: An array of paths to specify which Slice files should be included in this set.
    This field is required.

    - `addWellKnownTypes`: A boolean indicating whether to include the Slice definitions contained in the [IceRPC Slice](https://github.com/icerpc/icerpc-slice) repository.
    These types are commonly used in applications utilizing Slice.
    Defaults to `true`.

If you do not specify any configuration sets, the extension will default to using the project's root directory for `paths`.

**Note:** the language server only works with `.slice` files, and will ignore any `.ice` files in your project. The above settings are only meaningful for projects using `.slice` files.

### Example

Below is an example `settings.json` file which configures 2 separate Slice projects:

```json
{
    "slice.languageServer.enabled": true,
    "slice.configurations": [
        {
            "paths": [
                "path/to/slice/directory"
            ]
        },
        {
            "addWellKnownTypes": false,
            "paths": [
                "path/to/specific/file.slice",
                "/absolute/path/to/other/slice/files",
            ]
        }
    ]
}
```
