import * as assert from 'assert';

import * as vscode from 'vscode';
import { getDocUri, activate, doc, getedits } from './helper';

// You can import and use all API from the 'vscode' module
// as well as import your extension to test it
// import * as myExtension from '../../extension';

suite('Extension Test Suite', () => {
	vscode.window.showInformationMessage('Start all tests.');

	const docUri = getDocUri('applyedits.sol');

	test('Testing for apply edit', async () => {
		await testcommand(docUri);
	});
});


async function testcommand(docUri: vscode.Uri){
	
	await activate(docUri);

	let val = await vscode.commands.executeCommand('slang-ex.applyedit');

	assert.equal(getedits(), '42\n');
}