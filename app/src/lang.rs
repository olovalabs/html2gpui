//! Language identification for open buffers plus LSP metadata shown in the
//! status bar.

use std::path::Path;

/// Tree-sitter language id used by the highlighter for this file
/// (`"rust"`, `"python"`, …), or `None` for unrecognized files.
pub fn language_for(path: &Path) -> Option<&'static str> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match name {
        "Dockerfile" | "Containerfile" => return Some("dockerfile"),
        "Makefile" => return Some("make"),
        _ => {}
    }
    Some(match ext.as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "hpp" => "cpp",
        "java" => "java",
        "rb" => "ruby",
        "sh" | "bash" => "bash",
        "php" => "php",
        "swift" => "swift",
        "kt" => "kotlin",
        "scala" => "scala",
        "lua" => "lua",
        "zig" => "zig",
        "cs" => "c-sharp",
        "sql" => "sql",
        "r" => "r",
        "xml" => "xml",
        _ => return None,
    })
}

/// The language server binary Zed uses by default for this language.
pub fn lsp_binary_for(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "rust" => "rust-analyzer",
        "go" => "gopls",
        "python" => "basedpyright-langserver",
        "c" | "cpp" => "clangd",
        "javascript" | "typescript" => "typescript-language-server",
        "lua" => "lua-language-server",
        "zig" => "zls",
        "ruby" => "ruby-lsp",
        "java" => "jdtls",
        "php" => "intelephense",
        "bash" => "bash-language-server",
        "yaml" => "yaml-language-server",
        _ => return None,
    })
}

/// Human-readable LSP availability for the status bar.
pub fn lsp_status(path: &Path) -> String {
    let Some(lang) = language_for(path) else {
        return "plain text".into();
    };
    match lsp_binary_for(lang) {
        Some(bin) if binary_on_path(bin) => format!("{lang} · LSP ready ({bin})"),
        Some(bin) => format!("{lang} · LSP server '{bin}' not found on PATH"),
        None => format!("{lang} · no default LSP"),
    }
}

fn binary_on_path(binary: &str) -> bool {
    let exe = if cfg!(windows) && !binary.ends_with(".exe") {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    };
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .any(|dir| dir.join(binary).is_file() || dir.join(&exe).is_file())
        })
        .unwrap_or(false)
}
