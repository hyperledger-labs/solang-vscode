'use strict';
import {
    createConnection, IConnection,
    IPCMessageReader, IPCMessageWriter,
    TextDocuments, InitializeResult,
    Files, Diagnostic,
    TextDocumentPositionParams,
    CompletionItem, Location, SignatureHelp,
} from 'vscode-languageserver';
import Uri from 'vscode-uri';

// import * as path from 'path';
// Create a connection for the server
const connection: IConnection = createConnection(
    new IPCMessageReader(process),
    new IPCMessageWriter(process));

console.log = connection.console.log.bind(connection.console);
console.error = connection.console.error.bind(connection.console);

const documents: TextDocuments = new TextDocuments();

let rootPath: string;

documents.listen(connection);

connection.listen();
