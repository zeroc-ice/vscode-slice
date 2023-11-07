// Copyright (c) ZeroC, Inc.

import { workspace, ExtensionContext, window } from "vscode";
import {
  Executable,
  LanguageClient,
  LanguageClientOptions,
  RevealOutputChannelOn,
  ServerOptions,
} from "vscode-languageclient/node";

// Create an output channel for the language server's trace information.
const traceOutputChannel = window.createOutputChannel(
  "Slice Language Server trace"
);

// The language client.
let client: LanguageClient | undefined;

/**
 * Create a new instance of LanguageClient.
 * @param {ServerOptions} serverOptions - The server options.
 * @param {LanguageClientOptions} clientOptions - The client options.
 * @returns {LanguageClient} - The created language client.
 */
const createClient = (
  serverOptions: ServerOptions,
  clientOptions: LanguageClientOptions
) => {
  return new LanguageClient(
    "slice-language-server",
    "Slice Language Server",
    serverOptions,
    clientOptions
  );
};

/**
 * Set up handling for configuration changes.
 * @param {LanguageClient} client - The language client.
 */
const handleConfigurationChanges = (client: LanguageClient) => {
  workspace.onDidChangeConfiguration((event) => {
    if (event.affectsConfiguration("slice-language-server")) {
      // Retrieve the updated configuration settings.
      const config = workspace.getConfiguration("slice-language-server");
      const searchDirectory = config.get<string>("searchDirectory");

      // Send the "workspace/didChangeConfiguration" notification to the server with the updated settings.
      client.sendNotification("workspace/didChangeConfiguration", {
        settings: {
          "slice-language-server": { searchDirectory },
        },
      });
    }
  });
};

/**
 * Activate the extension.
 * @param {ExtensionContext} context - The extension context.
 */
export async function activate(context: ExtensionContext) {
  try {
    traceOutputChannel.appendLine("Activating extension...");

    // Determine the platform and architecture, then set the command
    let command: string;
    const serverPath =
      context.extensionPath + process.env.SERVER_PATH ||
      "slice-language-server/";
    const isProduction = process.env.NODE_ENV === "production";
    if (isProduction) {
      switch (process.platform) {
        case "darwin": // macOS
          command = `${serverPath}${
            process.arch === "arm64" ? "aarch64" : "x86_64"
          }-apple-darwin/release/slice-language-server`;
          break;
        case "win32": // Windows
          command = `${serverPath}x86_64-pc-windows-msvc/release/slice-language-server.exe`;
          break;
        case "linux": // Linux
          command = `${serverPath}x86_64-unknown-linux-gnu/release/slice-language-server`;
          break;
        default:
          throw new Error(`Unsupported platform: ${process.platform}`);
      }
    } else {
      traceOutputChannel.appendLine(`FOOO: ${command}`);
    }

    const run: Executable = {
      command,
      options: { env: { ...process.env, RUST_LOG: "debug" } },
    };

    const serverOptions: ServerOptions = { run, debug: run };

    // Configure the language client options.
    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ scheme: "file", language: "slice" }],
      synchronize: {
        fileEvents: workspace.createFileSystemWatcher("**/.clientrc"),
      },
      traceOutputChannel,
      outputChannel: traceOutputChannel,
      revealOutputChannelOn: RevealOutputChannelOn.Never,
    };

    // Create and start the language client.
    client = createClient(serverOptions, clientOptions);
    traceOutputChannel.appendLine("Language client created");

    // Set up configuration change handling.
    handleConfigurationChanges(client);

    // Start the client.
    await client.start();
    traceOutputChannel.appendLine("Client started");
  } catch (error) {
    traceOutputChannel.appendLine(`Failed to start client: ${error}`);
    window.showErrorMessage(
      "Slice Language Server failed to start. See the trace for more information."
    );
  }
}

/**
 * Deactivate the extension.
 * @returns {Promise<void>} - A promise that resolves when the client has stopped.
 */
export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}
