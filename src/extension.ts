// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import * as path from 'path';

import * as cp from 'child_process';
import * as rpc from 'vscode-jsonrpc';

import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind, InitializeRequest, InitializeParams} from 'vscode-languageclient';

import { workspace, WorkspaceFolder} from 'vscode';
import { create } from 'domain';
import { createConnection } from 'net';

// this method is called when your extension is activated
// your extension is activated the very first time the command is executed
export function activate(context: vscode.ExtensionContext) {
	//const ws: WorkspaceFolder[] = workspace.workspaceFolders;

	// Use the console to output diagnostic information (console.log) and errors (console.error)
	// This line of code will only be executed once when your extension is activated
	console.log('Congratulations, your extension "slang-ex" is now active!');

	// The command has been defined in the package.json file
	// Now provide the implementation of the command with registerCommand
	// The commandId parameter must match the command field in package.json
	let disposable = vscode.commands.registerCommand('slang-ex.helloWorld', () => {
		// The code you place here will be executed every time your command is executed

		// Display a message box to the user
		vscode.window.showInformationMessage('Hello World from slang_ex!');
	});

	context.subscriptions.push(disposable);

	let childprocess = cp.spawn('node',[path.join(__dirname, 'dummy.js')]);

	let connection = rpc.createMessageConnection(
		//new rpc.StreamMessageReader(process.stdout),
		//new rpc.StreamMessageWriter(process.stdin)
		new rpc.SocketMessageReader(),
		new rpc.SocketMessageWriter()
	);

	//connection = createConnection(rpc.StreamMessageReader(), rpc.StreamMessageWriter(),);

	connection.listen();

	const serverModule = path.join(__dirname, 'server/server.js');
	console.log(__dirname);
	const serverOptions: ServerOptions = {
		debug: {
			module: serverModule,
			options: {
				execArgv: ['--nolazy', '--inspect=6009'],
			},
			transport: TransportKind.socket,
		},
		run: {
			module: serverModule,
			transport: TransportKind.socket,
		}
	};

	const clientoptions: LanguageClientOptions = {
		documentSelector: [
			{	language: 'solidity', scheme: 'file' },
			{	language: 'solidity', scheme: 'untitled' },
		]
	};
	

	const init: InitializeParams = {
		rootUri: null,
		processId: 1,
		capabilities: {},
		workspaceFolders: null,
	};

	let disposable1 = vscode.commands.registerCommand('slang-ex.sendfirstcode', () => {
		connection.sendNotification('something interesting');
		connection.sendRequest(InitializeRequest.type, init );
		console.log('sent request\n');
	});
	context.subscriptions.push(disposable1);


	//let clientdispos;

	//if(ws) {
	let	clientdispos = new LanguageClient(
			'solidity',
			'Soliditiy language server extension',
			serverOptions,
			clientoptions).start();
	//}
	context.subscriptions.push(clientdispos);

}

// this method is called when your extension is deactivated
export function deactivate() {}
