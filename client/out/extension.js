"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.deactivate = exports.activate = void 0;
const vscode_1 = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
function activate(context) {
    const traceOutputChannel = vscode_1.window.createOutputChannel("Slice Language Server trace");
    traceOutputChannel.appendLine("Activating extension...");
    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    const command = process.env.SERVER_PATH || "slice-language-server";
    const run = {
        command,
        options: {
            env: {
                ...process.env,
                // eslint-disable-next-line @typescript-eslint/naming-convention
                RUST_LOG: "debug",
            },
        },
    };
    const serverOptions = {
        run,
        debug: run,
    };
    // Options to control the language client
    // If the extension is launched in debug mode then the debug server options are used
    // Otherwise the run options are used
    // Options to control the language client
    let clientOptions = {
        // Register the server for slice documents
        documentSelector: [{ scheme: "file", language: "slice" }],
        synchronize: {
            // Notify the server about file changes to '.clientrc files contained in the workspace
            fileEvents: vscode_1.workspace.createFileSystemWatcher("**/.clientrc"),
        },
        traceOutputChannel,
        outputChannel: traceOutputChannel,
        revealOutputChannelOn: node_1.RevealOutputChannelOn.Never,
    };
    // Create the language client and start the client.
    client = new node_1.LanguageClient("slice-language-server", "Slice Language Server", serverOptions, clientOptions);
    traceOutputChannel.appendLine("Language client created");
    traceOutputChannel.appendLine("Starting client...");
    client.start().then(() => {
        traceOutputChannel.appendLine("Client started");
        console.log("Client started");
    });
}
exports.activate = activate;
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
exports.deactivate = deactivate;
//# sourceMappingURL=extension.js.map