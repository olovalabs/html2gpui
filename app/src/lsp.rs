//! Ready-made Language Server Protocol (LSP) client manager.
//!
//! Spawns and communicates with standard language servers (e.g.
//! `typescript-language-server`, `rust-analyzer`, `gopls`, `pyright`) over
//! stdio using JSON-RPC and `lsp-types`. Diagnostics are streamed to the
//! workspace and rendered as real-time squiggly underlines.
//!
//! Besides the streaming notifications, the client supports synchronous
//! request/response pairs (`textDocument/completion`, `textDocument/hover`,
//! `textDocument/definition`, `textDocument/codeAction`,
//! `textDocument/formatting`, …) which are exposed to the editor through the
//! provider hooks of gpui-component's `InputState::lsp` — the same surface
//! Zed's editor uses to talk to language servers.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::ops::Range as StdRange;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, SharedString, Task, Window};
use gpui_component::input::{
    CodeActionProvider, CompletionProvider, DefinitionProvider, HoverProvider, InputState, Rope,
    RopeExt,
};
use lsp_types::{
    ClientCapabilities, CodeAction, CodeActionContext, CodeActionOrCommand, CodeActionParams,
    CodeActionResponse, CodeActionTriggerKind, CompletionContext, CompletionParams,
    CompletionResponse, CompletionTriggerKind, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    FormattingOptions, GotoDefinitionResponse, Hover, InitializeParams, InitializedParams,
    Location, LocationLink, PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams,
    TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, TextDocumentSyncClientCapabilities, Uri,
    VersionedTextDocumentIdentifier,
};
use serde_json::{json, Value};

/// How long a request may take before we give up (servers that are
/// initializing a large project can be slow on the first request).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug)]
pub enum LspEvent {
    Diagnostics {
        path: PathBuf,
        diagnostics: Vec<lsp_types::Diagnostic>,
    },
    #[allow(dead_code)]
    Status {
        lang: String,
        message: String,
    },
}

pub struct LspManager {
    clients: HashMap<String, Arc<LspClient>>,
    event_tx: async_channel::Sender<LspEvent>,
    event_rx: async_channel::Receiver<LspEvent>,
}

impl LspManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = async_channel::unbounded();
        Self {
            clients: HashMap::new(),
            event_tx,
            event_rx,
        }
    }

    pub fn event_receiver(&self) -> async_channel::Receiver<LspEvent> {
        self.event_rx.clone()
    }

    /// Ensure an LSP server is running for the given language and workspace root.
    pub fn ensure_server(
        &mut self,
        lang: &str,
        root_dir: Option<&Path>,
    ) -> Option<Arc<LspClient>> {
        if let Some(client) = self.clients.get(lang) {
            if client.is_alive() {
                return Some(client.clone());
            }
        }

        let binary = crate::lang::lsp_binary_for(lang)?;
        let client = Arc::new(LspClient::spawn(binary, lang, root_dir, self.event_tx.clone())?);
        self.clients.insert(lang.to_string(), client.clone());
        Some(client)
    }

    /// The running client for `lang`, if any (no spawning).
    pub fn client_for(&self, lang: &str) -> Option<Arc<LspClient>> {
        self.clients
            .get(lang)
            .cloned()
            .filter(|c| c.is_alive())
    }

    pub fn change_document(&mut self, path: &Path, lang: &str, text: &str) {
        if let Some(client) = self.clients.get(lang) {
            client.did_change(path, text);
        }
    }

    pub fn close_document(&mut self, path: &Path, lang: &str) {
        if let Some(client) = self.clients.get(lang) {
            client.did_close(path);
        }
    }

    /// True when an LSP client is running for `lang`.
    ///
    /// Lets the UI skip whole-buffer reads and coalescing work on every
    /// keystroke when no server exists for the language (e.g. plain text or
    /// a server binary that isn't installed).
    pub fn has_client(&self, lang: &str) -> bool {
        self.clients.contains_key(lang)
    }
}

pub struct LspClient {
    #[allow(dead_code)]
    pub lang: String,
    /// Outbound framed messages, drained by a dedicated writer thread so a
    /// slow language server (full stdin pipe) can never stall the UI thread.
    /// The channel is unbounded, so `send` never blocks.
    out: mpsc::Sender<Vec<u8>>,
    versions: Arc<Mutex<HashMap<PathBuf, i32>>>,
    is_alive: Arc<Mutex<bool>>,
    is_initialized: Arc<Mutex<bool>>,
    pending_opens: Arc<Mutex<Vec<(PathBuf, String, String)>>>,
    /// Documents whose text changed but haven't been synced yet: latest text
    /// per path. The writer thread flushes these as debounced full-document
    /// `didChange` messages, coalescing a fast typist's keystrokes into a
    /// single sync instead of one whole-document message per keystroke.
    pending_changes: Arc<Mutex<HashMap<PathBuf, String>>>,
    /// The most recent text we told the server for each open document. Used
    /// by providers to answer requests that need positions (code actions).
    last_texts: Arc<Mutex<HashMap<PathBuf, String>>>,
    /// Latest diagnostics per document (from publishDiagnostics), used to
    /// build the `CodeActionContext` of code-action requests.
    last_diagnostics: Arc<Mutex<HashMap<PathBuf, Vec<lsp_types::Diagnostic>>>>,
    /// Capabilities advertised by the server in its `initialize` response.
    server_capabilities: Arc<Mutex<Option<lsp_types::ServerCapabilities>>>,
    /// Monotonic request id allocator. Starts at 100 so the reserved
    /// `initialize` id (1) never collides with a routed response.
    next_id: AtomicI64,
    /// In-flight requests: id → channel that the reader thread delivers the
    /// response on.
    pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Value>>>>,
    child: Arc<Mutex<Option<Child>>>,
}

impl LspClient {
    pub fn spawn(
        binary: &str,
        lang: &str,
        root_dir: Option<&Path>,
        event_tx: async_channel::Sender<LspEvent>,
    ) -> Option<Self> {
        let bin_path = find_binary_on_path(binary);
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            if let Some(ref p) = bin_path {
                c.args(["/C", &p.to_string_lossy()]);
            } else {
                c.args(["/C", binary]);
            }
            c
        } else if let Some(ref p) = bin_path {
            Command::new(p)
        } else {
            Command::new(binary)
        };

        // Add standard stdio flags for language servers that require them
        match binary {
            "typescript-language-server" | "basedpyright-langserver" | "pyright-langserver" => {
                cmd.arg("--stdio");
            }
            "bash-language-server" => {
                cmd.arg("start");
            }
            "vscode-html-language-server" | "vscode-css-language-server" => {
                cmd.arg("--stdio");
            }
            _ => {}
        }

        if let Some(root) = root_dir {
            cmd.current_dir(root);
        }

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                eprintln!("[LSP] failed to spawn '{binary}': {e}");
                return None;
            }
        };

        let stdin = child.stdin.take()?;
        let stdout = child.stdout.take()?;

        let stdin_arc = Arc::new(Mutex::new(Some(stdin)));
        let is_alive = Arc::new(Mutex::new(true));
        let is_initialized = Arc::new(Mutex::new(false));
        let pending_opens = Arc::new(Mutex::new(Vec::new()));
        let versions = Arc::new(Mutex::new(HashMap::new()));
        let pending_changes: Arc<Mutex<HashMap<PathBuf, String>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let last_texts: Arc<Mutex<HashMap<PathBuf, String>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let last_diagnostics: Arc<Mutex<HashMap<PathBuf, Vec<lsp_types::Diagnostic>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let server_capabilities: Arc<Mutex<Option<lsp_types::ServerCapabilities>>> =
            Arc::new(Mutex::new(None));
        let pending: Arc<Mutex<HashMap<i64, mpsc::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let child_arc = Arc::new(Mutex::new(Some(child)));

        // Framed-message channel drained by a dedicated writer thread. All
        // sends are non-blocking (unbounded mpsc), so the UI thread never
        // blocks on a language server that is slow to drain its stdin.
        let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();

        let client = Self {
            lang: lang.to_string(),
            out: out_tx.clone(),
            versions: versions.clone(),
            is_alive: is_alive.clone(),
            is_initialized: is_initialized.clone(),
            pending_opens: pending_opens.clone(),
            pending_changes: pending_changes.clone(),
            last_texts: last_texts.clone(),
            last_diagnostics: last_diagnostics.clone(),
            server_capabilities: server_capabilities.clone(),
            next_id: AtomicI64::new(100),
            pending: pending.clone(),
            child: child_arc,
        };

        // Send initialize request
        client.send_initialize(root_dir);

        // Writer thread: writes framed messages to the child's stdin and,
        // between messages, flushes coalesced didChange documents (debounced
        // ~120 ms). Since this is its own OS thread, a blocked `write_all`
        // (full pipe) costs the UI thread nothing.
        {
            let is_alive_w = is_alive.clone();
            let stdin_for_write = stdin_arc.clone();
            let pending_w = pending_changes.clone();
            let versions_w = versions.clone();
            thread::spawn(move || {
                loop {
                    match out_rx.recv_timeout(Duration::from_millis(120)) {
                        Ok(bytes) => write_to_stdin(&stdin_for_write, &bytes),
                        Err(RecvTimeoutError::Timeout) => {
                            flush_pending_changes(&stdin_for_write, &pending_w, &versions_w);
                        }
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                    if !*is_alive_w.lock().unwrap() {
                        flush_pending_changes(&stdin_for_write, &pending_w, &versions_w);
                        break;
                    }
                }
            });
        }

        // Spawn stdout reader thread
        let lang_str = lang.to_string();
        let is_alive_clone = is_alive.clone();
        let is_init_clone = is_initialized.clone();
        let out_for_init = out_tx.clone();
        let pending_for_init = pending_opens.clone();
        let versions_for_init = versions.clone();
        let last_texts_r = last_texts.clone();
        let last_diag_r = last_diagnostics.clone();
        let caps_r = server_capabilities.clone();
        let pending_r = pending.clone();

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while *is_alive_clone.lock().unwrap() {
                match read_message(&mut reader) {
                    Ok(Some(mut msg)) => {
                        let id = msg.get("id").and_then(Value::as_i64);
                        let method = msg
                            .get("method")
                            .and_then(Value::as_str)
                            .map(str::to_string);

                        match (id, method.as_deref()) {
                            // The `initialize` response (id 1, no method).
                            (Some(1), None) => {
                                // Remember what this server can do so providers
                                // can skip unsupported features.
                                *caps_r.lock().unwrap() = msg
                                    .get("result")
                                    .and_then(|r| r.get("capabilities"))
                                    .and_then(|c| {
                                        serde_json::from_value::<
                                            lsp_types::ServerCapabilities,
                                        >(c.clone())
                                        .ok()
                                    });

                                *is_init_clone.lock().unwrap() = true;

                                // Send initialized notification
                                let initialized = json!({
                                    "jsonrpc": "2.0",
                                    "method": "initialized",
                                    "params": InitializedParams {}
                                });
                                send_framed(&out_for_init, &initialized);

                                // Drain and send all pending did_open documents
                                let pendings: Vec<_> = {
                                    let mut guard = pending_for_init.lock().unwrap();
                                    guard.drain(..).collect()
                                };

                                for (path, lang, text) in pendings {
                                    if let Some(uri) = path_to_uri(&path) {
                                        versions_for_init.lock().unwrap().insert(path.clone(), 1);
                                        last_texts_r.lock().unwrap().insert(path, text.clone());
                                        let params = DidOpenTextDocumentParams {
                                            text_document: TextDocumentItem {
                                                uri,
                                                language_id: lang,
                                                version: 1,
                                                text,
                                            },
                                        };
                                        let open_msg = json!({
                                            "jsonrpc": "2.0",
                                            "method": "textDocument/didOpen",
                                            "params": params
                                        });
                                        send_framed(&out_for_init, &open_msg);
                                    }
                                }
                            }
                            // A response to one of our requests.
                            (Some(req_id), None) => {
                                if let Some(tx) = pending_r.lock().unwrap().remove(&req_id) {
                                    let _ = tx.send(msg);
                                }
                            }
                            // A server-initiated request: we don't implement
                            // any, so answer method-not-found so servers
                            // don't wait forever.
                            (Some(req_id), Some(_)) => {
                                let err = json!({
                                    "jsonrpc": "2.0",
                                    "id": req_id,
                                    "error": {
                                        "code": -32601,
                                        "message": "method not found"
                                    }
                                });
                                send_framed(&out_for_init, &err);
                            }
                            // A notification from the server.
                            (None, Some(_)) => {
                                handle_incoming_message(&mut msg, &event_tx, &last_diag_r);
                            }
                            (None, None) => {}
                        }
                    }
                    Ok(None) => {
                        break;
                    }
                    Err(e) => {
                        eprintln!("[LSP: {lang_str}] read error: {e}");
                        break;
                    }
                }
            }
            *is_alive_clone.lock().unwrap() = false;
        });

        Some(client)
    }

    pub fn is_alive(&self) -> bool {
        *self.is_alive.lock().unwrap()
    }

    /// True once the initialize handshake finished and the process is alive.
    fn is_ready(&self) -> bool {
        *self.is_initialized.lock().unwrap() && *self.is_alive.lock().unwrap()
    }

    /// The capabilities the server advertised in its initialize response.
    pub fn capabilities(&self) -> Option<lsp_types::ServerCapabilities> {
        self.server_capabilities.lock().unwrap().clone()
    }

    /// True when the server advertised a capability (or hasn't told us yet —
    /// requests are still safe because they no-op until initialized).
    pub fn supports(&self, pred: impl FnOnce(&lsp_types::ServerCapabilities) -> bool) -> bool {
        self.capabilities().as_ref().map(pred).unwrap_or(true)
    }

    /// The most recent text synced for `path`, used to answer requests that
    /// need line/character positions.
    pub fn last_text(&self, path: &Path) -> Option<String> {
        self.last_texts.lock().unwrap().get(path).cloned()
    }

    #[allow(deprecated)]
    fn send_initialize(&self, root_dir: Option<&Path>) {
        let mut params = InitializeParams::default();
        params.process_id = Some(std::process::id());
        params.root_uri = root_dir.and_then(path_to_uri);
        params.capabilities = ClientCapabilities {
            text_document: Some(TextDocumentClientCapabilities {
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities {
                    related_information: Some(true),
                    version_support: Some(true),
                    code_description_support: Some(true),
                    data_support: Some(true),
                    ..Default::default()
                }),
                synchronization: Some(TextDocumentSyncClientCapabilities {
                    dynamic_registration: Some(true),
                    will_save: Some(false),
                    will_save_wait_until: Some(false),
                    did_save: Some(true),
                }),
                completion: Some(lsp_types::CompletionClientCapabilities {
                    completion_item: Some(lsp_types::CompletionItemCapability {
                        snippet_support: Some(true),
                        documentation_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                hover: Some(lsp_types::HoverClientCapabilities {
                    content_format: Some(vec![lsp_types::MarkupKind::Markdown]),
                    ..Default::default()
                }),
                definition: Some(lsp_types::GotoCapability {
                    dynamic_registration: Some(true),
                    link_support: Some(true),
                }),
                code_action: Some(lsp_types::CodeActionClientCapabilities {
                    code_action_literal_support: Some(lsp_types::CodeActionLiteralSupport {
                        code_action_kind: lsp_types::CodeActionKindLiteralSupport {
                            value_set: vec![
                                lsp_types::CodeActionKind::QUICKFIX.as_str().to_string(),
                                lsp_types::CodeActionKind::REFACTOR.as_str().to_string(),
                                lsp_types::CodeActionKind::REFACTOR_EXTRACT.as_str().to_string(),
                                lsp_types::CodeActionKind::REFACTOR_INLINE.as_str().to_string(),
                                lsp_types::CodeActionKind::REFACTOR_REWRITE.as_str().to_string(),
                                lsp_types::CodeActionKind::SOURCE.as_str().to_string(),
                                lsp_types::CodeActionKind::SOURCE_ORGANIZE_IMPORTS.as_str().to_string(),
                            ],
                        },
                    }),
                    ..Default::default()
                }),
                formatting: Some(lsp_types::DocumentFormattingClientCapabilities {
                    dynamic_registration: Some(true),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        params.client_info = Some(lsp_types::ClientInfo {
            name: "Olova Editor".into(),
            version: Some("0.1.0".into()),
        });

        if self.lang == "typescript" || self.lang == "javascript" {
            if let Some(ts_path) = find_tsserver_path() {
                params.initialization_options = Some(json!({
                    "tsserver": {
                        "path": ts_path.to_string_lossy()
                    }
                }));
            }
        }

        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": params
        });

        self.send_payload(&init_req);
    }

    pub fn did_open(&self, path: &Path, lang: &str, text: &str) {
        self.last_texts
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), text.to_string());

        if !*self.is_initialized.lock().unwrap() {
            // Queue until initialize handshake completes
            self.pending_opens
                .lock()
                .unwrap()
                .push((path.to_path_buf(), lang.to_string(), text.to_string()));
            return;
        }

        let Some(uri) = path_to_uri(path) else {
            return;
        };
        self.versions.lock().unwrap().insert(path.to_path_buf(), 1);

        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: lang.to_string(),
                version: 1,
                text: text.to_string(),
            },
        };

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": params
        });

        self.send_payload(&msg);
    }

    pub fn did_change(&self, path: &Path, text: &str) {
        if !*self.is_initialized.lock().unwrap() {
            return;
        }
        self.last_texts
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), text.to_string());

        // Coalesce: remember only the latest text per document. The writer
        // thread flushes pending changes as debounced full-document syncs,
        // so a fast typist costs one network round-trip every ~120 ms
        // instead of one whole-document serialization + write per keystroke.
        self.pending_changes
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), text.to_string());
    }

    pub fn did_close(&self, path: &Path) {
        let Some(uri) = path_to_uri(path) else {
            return;
        };
        self.versions.lock().unwrap().remove(path);
        self.last_texts.lock().unwrap().remove(path);
        self.last_diagnostics.lock().unwrap().remove(path);
        // Drop any not-yet-flushed edit for the document so a coalesced
        // didChange can never race the didClose.
        self.pending_changes.lock().unwrap().remove(path);

        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        };

        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": params
        });

        self.send_payload(&msg);
    }

    /// Immediately sync `text` as the authoritative full document, dropping
    /// any not-yet-flushed coalesced edit. Call before a request so the
    /// server's copy matches what the user sees.
    pub fn sync_document(&self, path: &Path, text: &str) {
        if !self.is_ready() {
            return;
        }
        let Some(uri) = path_to_uri(path) else {
            return;
        };
        self.pending_changes.lock().unwrap().remove(path);
        self.last_texts
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), text.to_string());

        let version = {
            let mut versions_guard = self.versions.lock().unwrap();
            let v = versions_guard.entry(path.to_path_buf()).or_insert(0);
            *v += 1;
            *v
        };
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        };
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": params
        });
        self.send_payload(&msg);
    }

    /// Send a JSON-RPC request and block (up to `timeout`) for its response.
    /// Returns the `result` field, or `None` on timeout / server error /
    /// disconnect. Safe to call from background threads; never call from the
    /// UI thread.
    pub fn request(&self, method: &str, params: Value, timeout: Duration) -> Option<Value> {
        if !self.is_ready() {
            return None;
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel::<Value>();
        self.pending.lock().unwrap().insert(id, tx);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        self.send_payload(&msg);
        match rx.recv_timeout(timeout) {
            Ok(resp) => resp.get("result").cloned(),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                None
            }
        }
    }

    /// [`Self::sync_document`] followed by [`Self::request`] with the
    /// default timeout. The server's answer is guaranteed to be computed
    /// against exactly `text`.
    pub fn request_with_text(
        &self,
        path: &Path,
        text: &str,
        method: &str,
        params: Value,
    ) -> Option<Value> {
        self.sync_document(path, text);
        self.request(method, params, REQUEST_TIMEOUT)
    }

    /// `textDocument/formatting` for the whole document.
    pub fn format_document(&self, path: &Path, text: &str) -> Option<Vec<lsp_types::TextEdit>> {
        if !self.supports(|c| c.document_formatting_provider.is_some()) {
            return None;
        }
        let uri = path_to_uri(path)?;
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier::new(uri),
            options: FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                properties: Default::default(),
                trim_trailing_whitespace: None,
                insert_final_newline: None,
                trim_final_newlines: None,
            },
            work_done_progress_params: Default::default(),
        };
        let params = serde_json::to_value(params).ok()?;
        let response = self.request_with_text(path, text, "textDocument/formatting", params)?;
        serde_json::from_value::<Vec<lsp_types::TextEdit>>(response).ok()
    }

    fn send_payload(&self, val: &Value) {
        send_framed(&self.out, val);
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Best-effort graceful shutdown: the writer thread may flush these
        // before the child is killed.
        *self.is_alive.lock().unwrap() = false;
        let shutdown = json!({"jsonrpc": "2.0", "id": 999_999, "method": "shutdown", "params": null});
        if let Some(bytes) = frame_payload(&shutdown) {
            let _ = self.out.send(bytes);
        }
        let exit = json!({"jsonrpc": "2.0", "method": "exit"});
        if let Some(bytes) = frame_payload(&exit) {
            let _ = self.out.send(bytes);
        }
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
}

/// Attach the standard LSP provider set (completions, hover, go-to-definition,
/// code actions) to an editor. Call once per editor after its server is
/// ensured; the providers talk to `client` and identify documents by `path`.
pub fn attach_lsp_providers(state: &mut InputState, client: Arc<LspClient>, path: PathBuf) {
    state.lsp.completion_provider = Some(Rc::new(LspCompletionProvider {
        client: client.clone(),
        path: path.clone(),
    }));
    state.lsp.hover_provider = Some(Rc::new(LspHoverProvider {
        client: client.clone(),
        path: path.clone(),
    }));
    state.lsp.definition_provider = Some(Rc::new(LspDefinitionProvider {
        client: client.clone(),
        path: path.clone(),
    }));
    state.lsp.code_action_providers = vec![Rc::new(LspCodeActionProvider { client, path })];
}

/// Characters that fire a completion request while typing. Mirrors the
/// trigger set of VS Code / Zed: word characters plus common punctuation
/// that usually starts a member access or argument list.
pub fn is_completion_trigger_char(c: char) -> bool {
    c.is_alphanumeric()
        || matches!(
            c,
            '_' | '.' | '$' | ':' | '/' | '<' | '"' | '#' | '@' | '-' | '>' | '(' | '[' | '{'
        )
}

// -- Providers --------------------------------------------------------------

/// `textDocument/completion` + inline (ghost text) completions.
pub struct LspCompletionProvider {
    client: Arc<LspClient>,
    path: PathBuf,
}

impl CompletionProvider for LspCompletionProvider {
    fn completions(
        &self,
        text: &Rope,
        offset: usize,
        trigger: CompletionContext,
        _window: &mut Window,
        cx: &mut Context<InputState>,
    ) -> Task<anyhow::Result<CompletionResponse>> {
        if !self.client.supports(|c| c.completion_provider.is_some()) {
            return Task::ready(Ok(CompletionResponse::Array(vec![])));
        }
        let client = self.client.clone();
        let path = self.path.clone();
        let text_str = text.to_string();
        let position = text.offset_to_position(offset);

        // The library hands us the whole query text typed since the menu
        // opened; the protocol wants a single trigger character, so only
        // forward it when it is exactly one char.
        let trigger_character = trigger.trigger_character.filter(|c| c.chars().count() == 1);
        let trigger_kind = if trigger_character.is_some() {
            trigger.trigger_kind
        } else {
            CompletionTriggerKind::INVOKED
        };

        cx.background_spawn(async move {
            let Some(uri) = path_to_uri(&path) else {
                return Ok(CompletionResponse::Array(vec![]));
            };
            let params = CompletionParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier::new(uri),
                    position,
                ),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: Some(CompletionContext {
                    trigger_kind,
                    trigger_character,
                }),
            };
            let Ok(params) = serde_json::to_value(params) else {
                return Ok(CompletionResponse::Array(vec![]));
            };
            let response =
                client.request_with_text(&path, &text_str, "textDocument/completion", params);
            Ok(response
                .and_then(|v| serde_json::from_value::<CompletionResponse>(v).ok())
                .unwrap_or(CompletionResponse::Array(vec![])))
        })
    }

    fn is_completion_trigger(
        &self,
        _offset: usize,
        new_text: &str,
        _cx: &mut Context<InputState>,
    ) -> bool {
        new_text.chars().next().map(is_completion_trigger_char).unwrap_or(false)
    }
}

/// `textDocument/hover` — the library shows the result in a popover.
pub struct LspHoverProvider {
    client: Arc<LspClient>,
    path: PathBuf,
}

impl HoverProvider for LspHoverProvider {
    fn hover(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Option<Hover>>> {
        if !self.client.supports(|c| c.hover_provider.is_some()) {
            return Task::ready(Ok(None));
        }
        let client = self.client.clone();
        let path = self.path.clone();
        let text_str = text.to_string();
        let position = text.offset_to_position(offset);

        cx.background_spawn(async move {
            let Some(uri) = path_to_uri(&path) else {
                return Ok(None);
            };
            let params = TextDocumentPositionParams::new(TextDocumentIdentifier::new(uri), position);
            let Ok(params) = serde_json::to_value(params) else {
                return Ok(None);
            };
            let response = client.request_with_text(&path, &text_str, "textDocument/hover", params);
            Ok(response
                .and_then(|v| serde_json::from_value::<Option<Hover>>(v).ok())
                .flatten())
        })
    }
}

/// `textDocument/definition` — ctrl-hover underlines the symbol and F12
/// jumps (the library renders the link highlight and handles the jump).
pub struct LspDefinitionProvider {
    client: Arc<LspClient>,
    path: PathBuf,
}

impl DefinitionProvider for LspDefinitionProvider {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        if !self.client.supports(|c| c.definition_provider.is_some()) {
            return Task::ready(Ok(vec![]));
        }
        let client = self.client.clone();
        let path = self.path.clone();
        let text_str = text.to_string();
        let position = text.offset_to_position(offset);

        cx.background_spawn(async move {
            let Some(uri) = path_to_uri(&path) else {
                return Ok(vec![]);
            };
            let params = TextDocumentPositionParams::new(TextDocumentIdentifier::new(uri), position);
            let Ok(params) = serde_json::to_value(params) else {
                return Ok(vec![]);
            };
            let response = client.request_with_text(&path, &text_str, "textDocument/definition", params);
            let locations = response
                .and_then(|v| serde_json::from_value::<GotoDefinitionResponse>(v).ok())
                .map(|r| match r {
                    GotoDefinitionResponse::Scalar(loc) => vec![location_to_link(loc)],
                    GotoDefinitionResponse::Array(locs) => {
                        locs.into_iter().map(location_to_link).collect()
                    }
                    GotoDefinitionResponse::Link(links) => links,
                })
                .unwrap_or_default();
            Ok(locations)
        })
    }
}

/// `textDocument/codeAction` + `workspace/executeCommand`.
pub struct LspCodeActionProvider {
    client: Arc<LspClient>,
    path: PathBuf,
}

impl CodeActionProvider for LspCodeActionProvider {
    fn id(&self) -> SharedString {
        "LSP".into()
    }

    fn code_actions(
        &self,
        _state: Entity<InputState>,
        range: StdRange<usize>,
        _window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<CodeAction>>> {
        if !self.client.supports(|c| c.code_action_provider.is_some()) {
            return Task::ready(Ok(vec![]));
        }
        let client = self.client.clone();
        let path = self.path.clone();

        cx.background_spawn(async move {
            // The byte range refers to the text we last synced; rebuild the
            // rope so we can convert it to line/character positions.
            let Some(text) = client.last_text(&path) else {
                return Ok(vec![]);
            };
            let rope = Rope::from_str(&text);
            let start = rope.offset_to_position(range.start);
            let end = rope.offset_to_position(range.end);

            let Some(uri) = path_to_uri(&path) else {
                return Ok(vec![]);
            };
            let Some(params) = serde_json::to_value(CodeActionParams {
                text_document: TextDocumentIdentifier::new(uri),
                range: lsp_types::Range::new(start, end),
                context: CodeActionContext {
                    diagnostics: client
                        .last_diagnostics
                        .lock()
                        .unwrap()
                        .get(&path)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|d| {
                            // Keep only diagnostics overlapping the range.
                            !(d.range.end <= start || d.range.start >= end)
                        })
                        .collect(),
                    only: None,
                    trigger_kind: Some(CodeActionTriggerKind::INVOKED),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .ok()
            else {
                return Ok(vec![]);
            };

            let response =
                client.request_with_text(&path, &text, "textDocument/codeAction", params);
            let actions = response
                .and_then(|v| serde_json::from_value::<CodeActionResponse>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|item| match item {
                    CodeActionOrCommand::CodeAction(action) => action,
                    CodeActionOrCommand::Command(command) => CodeAction {
                        title: command.title.clone(),
                        kind: None,
                        diagnostics: None,
                        edit: None,
                        command: Some(command),
                        is_preferred: None,
                        disabled: None,
                        data: None,
                    },
                })
                .collect();
            Ok(actions)
        })
    }

    fn perform_code_action(
        &self,
        state: Entity<InputState>,
        action: CodeAction,
        _push_to_history: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<()>> {
        let client = self.client.clone();
        let path = self.path.clone();

        // Apply the action's workspace edits to the current document (the
        // library only has one document per editor; multi-file edits like
        // renames touch the other files on the next open).
        if let Some(edit) = action.edit {
            if let (Some(uri), Some(changes)) = (path_to_uri(&path), edit.changes) {
                if let Some((_, text_edits)) =
                    changes.iter().find(|(u, _)| u.as_str() == uri.as_str())
                {
                    let text_edits = text_edits.clone();
                    let state = state.downgrade();
                    let _ = window.spawn(cx, async move |cx| {
                        if let Some(state) = state.upgrade() {
                            let _ = state.update_in(cx, |state, window, cx| {
                                state.apply_lsp_edits(&text_edits, window, cx);
                            });
                        }
                    });
                }
            }
        }

        // Execute the action's command on the server.
        if let Some(command) = action.command {
            let command_name = command.command;
            let arguments = command.arguments.unwrap_or_default();
            return cx.background_spawn(async move {
                let params = json!({ "command": command_name, "arguments": arguments });
                let _ = client.request("workspace/executeCommand", params, REQUEST_TIMEOUT);
                Ok(())
            });
        }

        Task::ready(Ok(()))
    }
}

fn location_to_link(loc: Location) -> LocationLink {
    LocationLink {
        target_uri: loc.uri,
        target_range: loc.range,
        target_selection_range: loc.range,
        origin_selection_range: None,
    }
}

/// Serialize `val` as one framed (Content-Length headed) JSON-RPC message.
/// `None` when serialization fails.
fn frame_payload(val: &Value) -> Option<Vec<u8>> {
    let json_str = serde_json::to_string(val).ok()?;
    let body = json_str.as_bytes();
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut bytes = Vec::with_capacity(header.len() + body.len());
    bytes.extend_from_slice(header.as_bytes());
    bytes.extend_from_slice(body);
    Some(bytes)
}

/// Non-blocking enqueue of one framed message onto the writer channel.
fn send_framed(tx: &mpsc::Sender<Vec<u8>>, val: &Value) {
    if let Some(bytes) = frame_payload(val) {
        let _ = tx.send(bytes);
    }
}

/// Write framed bytes to the child's stdin (writer thread only).
fn write_to_stdin(stdin_arc: &Arc<Mutex<Option<ChildStdin>>>, bytes: &[u8]) {
    let mut stdin_guard = stdin_arc.lock().unwrap();
    if let Some(stdin) = stdin_guard.as_mut() {
        let _ = stdin.write_all(bytes);
        let _ = stdin.flush();
    }
}

/// Flush every coalesced `didChange` document as a full-document sync with a
/// monotonically increasing version. Runs on the writer thread, so any pipe
/// backpressure blocks a background thread, never the UI.
fn flush_pending_changes(
    stdin_arc: &Arc<Mutex<Option<ChildStdin>>>,
    pending: &Mutex<HashMap<PathBuf, String>>,
    versions: &Mutex<HashMap<PathBuf, i32>>,
) {
    let items: Vec<(PathBuf, String)> = {
        let mut guard = pending.lock().unwrap();
        guard.drain().collect()
    };
    if items.is_empty() {
        return;
    }
    for (path, text) in items {
        // The document was closed before the flush — drop the stale edit.
        let doc_open = versions.lock().unwrap().contains_key(&path);
        if !doc_open {
            continue;
        }
        let Some(uri) = path_to_uri(&path) else {
            continue;
        };
        let version = {
            let mut versions_guard = versions.lock().unwrap();
            let v = versions_guard.entry(path).or_insert(1);
            *v += 1;
            *v
        };
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        };
        let msg = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didChange",
            "params": params
        });
        if let Some(bytes) = frame_payload(&msg) {
            write_to_stdin(stdin_arc, &bytes);
        }
    }
}

pub fn path_to_uri(path: &Path) -> Option<Uri> {
    let url = url::Url::from_file_path(path).ok()?;
    url.to_string().parse().ok()
}

pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let url = url::Url::parse(uri.as_str()).ok()?;
    url.to_file_path().ok()
}

pub fn paths_match(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

pub fn find_tsserver_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let l = PathBuf::from(local);
        candidates.push(l.join(r"Zed\languages\vtsls\node_modules\typescript\lib\tsserver.js"));
        candidates.push(l.join(r"Zed\languages\json-language-server\node_modules\typescript\lib\tsserver.js"));
        candidates.push(l.join(r"Zed\languages\vscode-css-language-server\node_modules\typescript\lib\tsserver.js"));
        candidates.push(l.join(r"Programs\Microsoft VS Code\resources\app\extensions\node_modules\typescript\lib\tsserver.js"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let a = PathBuf::from(appdata);
        candidates.push(a.join(r"npm\node_modules\typescript\lib\tsserver.js"));
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let h = PathBuf::from(home);
        candidates.push(h.join(".local/share/zed/languages/vtsls/node_modules/typescript/lib/tsserver.js"));
        candidates.push(h.join(".vscode/extensions/node_modules/typescript/lib/tsserver.js"));
    }

    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
    }
    None
}

pub fn find_binary_on_path(binary: &str) -> Option<PathBuf> {
    const WINDOWS_EXTS: [&str; 3] = [".cmd", ".exe", ".bat"];
    let candidates: Vec<String> = if cfg!(windows) && !WINDOWS_EXTS.iter().any(|e| binary.ends_with(e)) {
        WINDOWS_EXTS.iter().map(|e| format!("{binary}{e}")).collect()
    } else {
        vec![binary.to_string()]
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            candidates.iter().find_map(|name| {
                let p = dir.join(name);
                if p.is_file() {
                    Some(p)
                } else {
                    None
                }
            })
        })
    })
}

fn read_message<R: BufRead>(reader: &mut R) -> std::io::Result<Option<Value>> {
    let mut content_length = None;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(len_str) = trimmed.strip_prefix("Content-Length:") {
            if let Ok(len) = len_str.trim().parse::<usize>() {
                content_length = Some(len);
            }
        }
    }

    let Some(length) = content_length else {
        return Ok(None);
    };

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    let val = serde_json::from_slice(&body).ok();
    Ok(val)
}

fn handle_incoming_message(
    msg: &mut Value,
    event_tx: &async_channel::Sender<LspEvent>,
    last_diag: &Mutex<HashMap<PathBuf, Vec<lsp_types::Diagnostic>>>,
) {
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        return;
    };

    if method == "textDocument/publishDiagnostics" {
        // `params` is taken (moved) out of `msg` instead of cloned — the
        // JSON body of a diagnostics push is the heaviest per-message cost.
        if let Some(params) = msg.get_mut("params") {
            if let Ok(mut pub_diag) =
                serde_json::from_value::<PublishDiagnosticsParams>(std::mem::take(params))
            {
                if let Some(path) = uri_to_path(&pub_diag.uri) {
                    last_diag
                        .lock()
                        .unwrap()
                        .insert(path.clone(), pub_diag.diagnostics.clone());
                    for diag in &mut pub_diag.diagnostics {
                        let source_str = diag.source.as_deref().unwrap_or("typescript");
                        let code_str = match &diag.code {
                            Some(lsp_types::NumberOrString::Number(n)) => format!(" ({n})"),
                            Some(lsp_types::NumberOrString::String(s)) => format!(" ({s})"),
                            None => String::new(),
                        };
                        diag.message = format!("`{source_str}{code_str}`\n\n{}", diag.message);
                    }
                    let _ = event_tx.try_send(LspEvent::Diagnostics {
                        path,
                        diagnostics: pub_diag.diagnostics,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_path_roundtrip() {
        let path = if cfg!(windows) {
            PathBuf::from(r"C:\Users\test\project\app.ts")
        } else {
            PathBuf::from("/home/test/project/app.ts")
        };
        let uri = path_to_uri(&path).expect("valid uri");
        let roundtrip = uri_to_path(&uri).expect("valid path");
        assert!(paths_match(&path, &roundtrip));
    }

    #[test]
    fn test_read_message_framing() {
        let payload = r#"{"jsonrpc":"2.0","method":"test","params":{}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{}", payload.len(), payload);
        let mut cursor = std::io::Cursor::new(framed.into_bytes());
        let msg = read_message(&mut cursor).expect("read ok").expect("some msg");
        assert_eq!(msg["method"], "test");
    }

    #[test]
    fn test_trigger_chars() {
        assert!(is_completion_trigger_char('a'));
        assert!(is_completion_trigger_char('_'));
        assert!(is_completion_trigger_char('.'));
        assert!(!is_completion_trigger_char(' '));
        assert!(!is_completion_trigger_char(';'));
    }

    #[test]
    fn test_live_typescript_diagnostics() {
        if find_binary_on_path("typescript-language-server").is_none() {
            println!("typescript-language-server not found, skipping");
            return;
        }

        let mut mgr = LspManager::new();
        let rx = mgr.event_receiver();
        let test_file = std::env::temp_dir().join("olova_test_bad.ts");
        let bad_code = "function test(x: number): string { return x; }";

        if let Some(client) = mgr.ensure_server("typescript", None) {
            client.did_open(&test_file, "typescript", bad_code);
        }

        // Wait up to 5 seconds for diagnostics
        let start = std::time::Instant::now();
        let mut got_diagnostics = false;
        while start.elapsed() < std::time::Duration::from_secs(5) {
            if let Ok(LspEvent::Diagnostics { path, diagnostics }) = rx.try_recv() {
                println!("Received diagnostics for {}: {:?}", path.display(), diagnostics);
                if paths_match(&path, &test_file) && !diagnostics.is_empty() {
                    got_diagnostics = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        println!("Live LSP test got_diagnostics: {got_diagnostics}");
    }
}
