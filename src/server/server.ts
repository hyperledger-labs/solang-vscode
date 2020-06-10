import {
    createConnection,
    Diagnostic,
    Range,
    DiagnosticSeverity
} from 'vscode-languageserver';
import { ProposedFeatures } from 'vscode-languageclient';
import * as rpc from 'vscode-jsonrpc';

let connection = createConnection(
    new rpc.StreamMessageReader(process.stdin),
    new rpc.StreamMessageWriter(process.stdout)
);

//const connection = createConnection();

connection.console.log(`Sample server running in node ${process.version}`);

/*
connection.onInitialize(() => {
    return {
        capabilities: null
    };
});
*/

function validate(): void {
    connection.sendDiagnostics({
        uri: '1',
        version: 1,
        diagnostics: [
            Diagnostic.create(Range.create(0,0,0, 10), 'Something is wrong here', DiagnosticSeverity.Warning)
        ]
    });
}

let notif = new rpc.NotificationType<string, void>('test notif');

connection.onNotification(notif, (param: string) => {
    console.log('notified\n');
    console.log(param);
});

connection.listen();