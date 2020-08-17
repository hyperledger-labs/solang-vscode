use jsonrpc_core::Result;
use serde_json::Value;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use solang::file_cache::FileCache;
use solang::parse_and_resolve;
use solang::Target;

use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use solang::sema::*;

use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Backend {
    state: Vec<usize>,
}

impl Backend {
    // Calculate the line and coloumn from the Loc offset recieved from the parser
    // Do a linear search till the correct offset location is matched
    fn file_offset_to_line_column(data: &str, loc: usize) -> (usize, usize) {
        let mut line_no = 0;
        let mut past_ch = 0;

        for (ind, c) in data.char_indices() {
            if c == '\n' {
                if ind == loc {
                    break;
                } else {
                    past_ch = ind + 1;
                    line_no += 1;
                }
            }
            if ind == loc {
                break;
            }
        }

        (line_no, loc - past_ch)
    }

    // Convert the diagnostic messages recieved from the solang to lsp diagnostics types.
    // Returns a vector of diagnostic messages for the client.
    fn convert_to_diagnostics(ns: ast::Namespace, filecache: &mut FileCache) -> Vec<Diagnostic> {
        let mut diagnostics_vec: Vec<Diagnostic> = Vec::new();

        for diag in ns.diagnostics {
            let pos = diag.pos.unwrap();

            let diagnostic = &diag;

            let sev = match diagnostic.level {
                ast::Level::Info => DiagnosticSeverity::Information,
                ast::Level::Warning => DiagnosticSeverity::Warning,
                ast::Level::Error => DiagnosticSeverity::Error,
                ast::Level::Debug => continue,
            };

            let fl = &ns.files[pos.0];

            let file_cont = filecache.get_file_contents(fl.as_str());

            let l1 = Backend::file_offset_to_line_column(&file_cont.as_str(), pos.1);

            let l2 = Backend::file_offset_to_line_column(&file_cont.as_str(), pos.2);

            let p1 = Position::new(l1.0 as u64, l1.1 as u64);

            let p2 = Position::new(l2.0 as u64, l2.1 as u64);

            let range = Range::new(p1, p2);

            let message_slice = &diag.message[..];

            diagnostics_vec.push(Diagnostic {
                range,
                message: message_slice.to_string(),
                severity: Some(sev),
                source: Some("solidity".to_string()),
                code: None,
                related_information: None,
                tags: None,
            });
        }

        diagnostics_vec
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: &Client, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::Incremental,
                )),
                hover_provider: Some(true),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                }),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: None,
                    retrigger_characters: None,
                    work_done_progress_options: Default::default(),
                }),
                document_highlight_provider: Some(true),
                workspace_symbol_provider: Some(true),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),
                workspace: Some(WorkspaceCapability {
                    workspace_folders: Some(WorkspaceFolderCapability {
                        supported: Some(true),
                        change_notifications: Some(
                            WorkspaceFolderCapabilityChangeNotifications::Bool(true),
                        ),
                    }),
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, client: &Client, _: InitializedParams) {
        client.log_message(MessageType::Info, "server initialized!");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_change_workspace_folders(
        &self,
        client: &Client,
        _: DidChangeWorkspaceFoldersParams,
    ) {
        client.log_message(MessageType::Info, "workspace folders changed!");
    }

    async fn did_change_configuration(&self, client: &Client, _: DidChangeConfigurationParams) {
        client.log_message(MessageType::Info, "configuration changed!");
    }

    async fn did_change_watched_files(&self, client: &Client, _: DidChangeWatchedFilesParams) {
        client.log_message(MessageType::Info, "watched files have changed!");
    }

    async fn execute_command(
        &self,
        client: &Client,
        _: ExecuteCommandParams,
    ) -> Result<Option<Value>> {
        client.log_message(MessageType::Info, "command executed!");
        Ok(None)
    }

    async fn did_open(&self, client: &Client, params: DidOpenTextDocumentParams) {
        client.log_message(MessageType::Info, "file opened!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_change(&self, client: &Client, params: DidChangeTextDocumentParams) {
        client.log_message(MessageType::Info, "file changed!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_save(&self, client: &Client, params: DidSaveTextDocumentParams) {
        client.log_message(MessageType::Info, "file saved!");

        let uri = params.text_document.uri;

        if let Ok(path) = uri.to_file_path() {
            let mut filecache = FileCache::new();

            let filecachepath = path.parent().unwrap();

            let tostrpath = filecachepath.to_str().unwrap();

            let mut p = PathBuf::new();

            p.push(tostrpath.to_string());

            filecache.add_import_path(p);

            let uri_string = uri.to_string();

            client.log_message(MessageType::Info, &uri_string);

            let os_str = path.file_name().unwrap();

            let ns = parse_and_resolve(os_str.to_str().unwrap(), &mut filecache, Target::Ewasm);

            let d = Backend::convert_to_diagnostics(ns, &mut filecache);

            client.publish_diagnostics(uri, d, None);
        }
    }

    async fn did_close(&self, client: &Client, _: DidCloseTextDocumentParams) {
        client.log_message(MessageType::Info, "file closed!");
    }

    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(Some(CompletionResponse::Array(vec![
            CompletionItem::new_simple("Hello".to_string(), "Some detail".to_string()),
            CompletionItem::new_simple("Bye".to_string(), "More detail".to_string()),
        ])))
    }

    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                "This is hover message from server!".to_string(),
            )),
            range: None,
        }))
    }
}
