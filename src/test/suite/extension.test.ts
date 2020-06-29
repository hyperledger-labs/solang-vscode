import * as assert from 'assert';

import * as vscode from 'vscode';
import { getDocUri, activate, doc, getedits } from './helper';

// You can import and use all API from the 'vscode' module
// as well as import your extension to test it
// import * as myExtension from '../../extension';

suite('Extension Test Suite', () => {
	vscode.window.showInformationMessage('Start all tests.');

	const docUri = getDocUri('applyedit.txt');

	test('Sample test', async () => {
		await testcommand(docUri);
	});
});


async function testcommand(docUri: vscode.Uri){
	await activate(docUri);

	console.log('activate ran');
	const val = await vscode.commands.executeCommand('slang-ex.applyedit');

	if(val === undefined){
		console.log('undefinded');
	}

	//console.log(val!.length);


	assert.equal(getedits(), '42\n');
}