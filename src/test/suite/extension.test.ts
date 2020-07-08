import * as assert from 'assert';

import * as vscode from 'vscode';
import { getDocUri, activate, doc, getedits } from './helper';
import { get } from 'http';

// You can import and use all API from the 'vscode' module
// as well as import your extension to test it
// import * as myExtension from '../../extension';

suite('Extension Test Suite', function () {
	vscode.window.showInformationMessage('Start all tests.');

	const docUri = getDocUri('applyedits.sol');

	this.timeout(20000);
	test('Testing for apply edit', async () => {
		await testcommand(docUri);
	});
});


async function testcommand(docUri: vscode.Uri){
	
	await activate(docUri);

	let val = await vscode.commands.executeCommand('slang-ex.applyedit');

	let res = await getedits();

	if(res){
		assert.equal(res.text, '42');
	}
	else{
		console.error('failed to initialize apply edit');
	}
}
