import * as path from "path";
import { workspace, ExtensionContext, window } from "vscode";

import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient;

export function activate(context: ExtensionContext) {
  const traceOutputChannel = window.createOutputChannel(
    "Slice Language Server trace"
  );
  traceOutputChannel.appendLine("Activating extension...");
  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used
  const command = process.env.SERVER_PATH || "slice-language-server";
  const run: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        // eslint-disable-next-line @typescript-eslint/naming-convention
        RUST_LOG: "debug",
      },
    },
  };
  const serverOptions: ServerOptions = {
    run,
    debug: run,
  };

  // Options to control the language client
  // If the extension is launched in debug mode then the debug server options are used
  // Otherwise the run options are used
  // Options to control the language client
  let clientOptions: LanguageClientOptions = {
    // Register the server for slice documents
    documentSelector: [{ scheme: "file", language: "slice" }],
    synchronize: {
      // Notify the server about file changes to '.clientrc files contained in the workspace
      fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
    },
    traceOutputChannel,
    outputChannel: traceOutputChannel,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
  };

  // Create the language client and start the client.
  client = new LanguageClient(
    "slice-language-server",
    "Slice Language Server",
    serverOptions,
    clientOptions
  );

  traceOutputChannel.appendLine("Language client created");
  traceOutputChannel.appendLine("Starting client...");
  client.start().then(() => {
    traceOutputChannel.appendLine("Client started");
    console.log("Client started");
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
