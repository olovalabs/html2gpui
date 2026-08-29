//! Language Server Protocol support, modelled on Zed's implementation.
//!
//! Zed splits this work across three crates; we keep the same separation of
//! concerns in three modules:
//!
//! | Zed | here | responsibility |
//! |---|---|---|
//! | `crates/node_runtime` | [`node`] | find Node, npm-install servers on demand |
//! | `crates/languages` (`LspAdapter` impls) | [`adapter`] | per-server binary, init options, settings, language ids |
//! | `crates/lsp` + `crates/project/src/lsp_store.rs` | [`client`] | JSON-RPC transport, document sync, diagnostics, providers |
//!
//! The end-to-end flow when a user opens `App.tsx` in a project with no
//! language servers installed:
//!
//! 1. `lang::language_for` → `"tsx"`.
//! 2. [`adapter::adapter_for_language`] → the `typescript-language-server`
//!    adapter.
//! 3. [`client::LspManager::ensure_server`] finds nothing installed and starts
//!    a background `npm install typescript-language-server typescript@6` into
//!    a private directory; the status bar shows *Installing…*.
//! 4. The install finishes → [`client::LspEvent::ServerReady`] → the workspace
//!    starts the server and opens every matching buffer.
//! 5. `initialize` → the client answers the server's `workspace/configuration`
//!    request from the adapter → the server starts publishing diagnostics.
//! 6. [`client::LspEvent::Diagnostics`] → squiggles in the editor.
//!
//! Steps 3 and 5 are the two that a naive implementation skips, and they are
//! exactly why "it works for TypeScript but not CSS" happens.

pub mod adapter;
pub mod client;
pub mod node;

pub use client::{attach_lsp_providers, paths_match, LspEvent, LspManager, ServerStatus};
