// Copyright (c) ZeroC, Inc.

import { workspace, ExtensionContext, window, Uri } from "vscode";
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
    "slice",
    "Slice Language Server",
    serverOptions,
    clientOptions
  );
};

let restartLanguageServer = async (context: ExtensionContext) => {
  if (client) {
    await client.stop();
    client = undefined;
  }

  await activate(context);
};

/**
 * Set up handling for configuration changes.
 */
const handleConfigurationChanges = (context: ExtensionContext) => {
  workspace.onDidChangeConfiguration(async (event) => {
    if (event.affectsConfiguration("slice")) {
      // Retrieve the updated configuration settings.
      const config = workspace.getConfiguration("slice");
      const enableLanguageServer = config.get<boolean>(
        "languageServer.enabled"
      );
      const referenceDirectories = config.get<string>("referenceDirectories");

      // Send the "workspace/didChangeConfiguration" notification to the server with the updated settings.
      if (client) {
        client.sendNotification("workspace/didChangeConfiguration", {
          settings: {
            slice: { enableLanguageServer, referenceDirectories },
          },
        });
      }
    }

    // Restart the language server if the languageServer.enabled setting has changed.
    if (event.affectsConfiguration("slice.languageServer.enabled")) {
      const config = workspace.getConfiguration("slice");
      const enabled = config.get<boolean>("languageServer.enabled"); // Corrected the key

      if (!enabled && client) {
        traceOutputChannel.appendLine(
          "Disabling language server as per configuration change..."
        );
        await client.stop();
        client = undefined;
      } else if (enabled && !client) {
        traceOutputChannel.appendLine(
          "Restarting language server as per configuration change..."
        );
        await restartLanguageServer(context);
      }
    }
  });
};

/**
 * Activate the extension.
 * @param {ExtensionContext} context - The extension context.
 */
export async function activate(context: ExtensionContext) {
  traceOutputChannel.appendLine("Activating extension...");

  // Don't activate the extension if languageServer.enabled is false.
  const config = workspace.getConfiguration("slice");
  const enableLanguageServer = config.get<boolean>("languageServer.enabled"); // Corrected the key
  if (!enableLanguageServer) {
    traceOutputChannel.appendLine("Language server disabled");
    return;
  }

  // Start the language server.
  try {
    // Determine the platform and architecture, then set the command
    let command: string;

    const isProduction = process.env.NODE_ENV === "production";
    const serverPath = isProduction
      ? context.extensionPath + process.env.SERVER_PATH
      : process.env.SERVER_PATH + "debug/slice-language-server";
    if (isProduction) {
      switch (process.platform) {
        case "darwin": // macOS
          command = `${serverPath}${
            process.arch === "arm64" ? "aarch64" : "x86_64"
          }-apple-darwin/release/slice-language-server`;
          break;
        case "win32": // Windows
          command = `${serverPath}x86_64-pc-windows-gnu/release/slice-language-server.exe`;
          break;
        case "linux": // Linux
          command = `${serverPath}${
            process.arch === "arm64" ? "aarch64" : "x86_64"
          }-unknown-linux-gnu/release/slice-language-server`;
          break;
        default:
          throw new Error(`Unsupported platform: ${process.platform}`);
      }
    } else {
      command = serverPath;
    }

    const run: Executable = {
      command,
      options: {
        env: { ...process.env, ...(isProduction ? {} : { RUST_LOG: "debug" }) },
      },
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

    // Start the client.
    await client.start();
    traceOutputChannel.appendLine("Client started");
  } catch (error) {
    traceOutputChannel.appendLine(`Failed to start client: ${error}`);
    window.showErrorMessage(
      "Slice Language Server failed to start. See the trace for more information."
    );
  }

  handleConfigurationChanges(context);
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
