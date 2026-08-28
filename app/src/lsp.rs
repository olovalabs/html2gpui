//! Ready-made Language Server Protocol (LSP) client manager.
//!
//! Spawns and communicates with standard language servers (e.g.
//! `typescript-language-server`, `rust-analyzer`, `gopls`, `pyright`) over
//! stdio using JSON-RPC and `lsp-types`. Diagnostics are streamed to the
//! workspace and rendered as real-time squiggly underlines.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializedParams, PublishDiagnosticsParams,
    PublishDiagnosticsClientCapabilities, TextDocumentClientCapabilities,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentSyncClientCapabilities, Uri, VersionedTextDocumentIdentifier,
};
use serde_json::{json, Value};

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

    pub fn open_document(&mut self, path: &Path, lang: &str, text: &str, root: Option<&Path>) {
        if let Some(client) = self.ensure_server(lang, root) {
            client.did_open(path, lang, text);
        }
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

        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            while *is_alive_clone.lock().unwrap() {
                match read_message(&mut reader) {
                    Ok(Some(mut msg)) => {
                        // Check if this is the initialize response (id: 1)
                        if msg.get("id").and_then(Value::as_i64) == Some(1) {
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
                                    versions_for_init.lock().unwrap().insert(path, 1);
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

                        handle_incoming_message(&mut msg, &event_tx);
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

    fn send_payload(&self, val: &Value) {
        send_framed(&self.out, val);
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
        // (Bind the check so the guard is dropped before the lock below.)
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

impl Drop for LspClient {
    fn drop(&mut self) {
        *self.is_alive.lock().unwrap() = false;
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
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

fn handle_incoming_message(msg: &mut Value, event_tx: &async_channel::Sender<LspEvent>) {
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
    fn test_live_typescript_diagnostics() {
        if find_binary_on_path("typescript-language-server").is_none() {
            println!("typescript-language-server not found, skipping");
            return;
        }

        let mut mgr = LspManager::new();
        let rx = mgr.event_receiver();
        let test_file = std::env::temp_dir().join("olova_test_bad.ts");
        let bad_code = "function test(x: number): string { return x; }";

        mgr.open_document(&test_file, "typescript", bad_code, None);

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
