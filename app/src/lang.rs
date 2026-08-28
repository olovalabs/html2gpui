//! Language identification for open buffers plus LSP server selection.
//!
//! `language_for` returns the tree-sitter language id used by the highlighter
//! (gpui-component's `LanguageRegistry` — see its `Language` enum for the
//! exact id list) and by the LSP layer. Detection is extension + filename
//! based, with a shebang fallback for extension-less scripts.
//!
//! `lsp_binary_for` maps a language to the language server binary Zed uses by
//! default for that language; the binary is looked up on `PATH` at runtime.

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
        "Makefile" | "makefile" | "GNUmakefile" => return Some("make"),
        "CMakeLists.txt" => return Some("cmake"),
        "Justfile" | "justfile" => return Some("make"),
        ".bashrc" | ".bash_profile" | ".zshrc" | ".profile" => return Some("bash"),
        _ => {}
    }
    if let Some(lang) = by_extension(&ext) {
        return Some(lang);
    }
    // Extension-less scripts: sniff the shebang from the first line.
    shebang_language(path)
}

fn by_extension(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" | "pyw" => "python",
        "html" | "htm" | "xhtml" => "html",
        "css" => "css",
        "scss" | "sass" => "css",
        "json" => "json",
        "jsonc" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" | "mdown" => "markdown",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "inl" => "cpp",
        "cs" => "csharp",
        "java" => "java",
        "rb" => "ruby",
        "sh" | "bash" | "zsh" => "bash",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "scala" | "sc" => "scala",
        "lua" => "lua",
        "zig" => "zig",
        "sql" => "sql",
        "r" => "r",
        "xml" | "xsl" | "xsd" | "svg" => "xml",
        "proto" => "proto",
        "graphql" | "gql" => "graphql",
        "ex" | "exs" => "elixir",
        "ejs" => "ejs",
        "erb" => "erb",
        "diff" | "patch" => "diff",
        "cmake" => "cmake",
        "vue" | "svelte" | "astro" => "html",
        "tex" => "latex",
        "dart" => "dart",
        "hs" => "haskell",
        "ml" => "ocaml",
        "fs" | "fsx" => "fsharp",
        "erl" => "erlang",
        "clj" | "cljs" => "clojure",
        "ps1" | "psm1" => "powershell",
        _ => return None,
    })
}

/// Detect the language of an extension-less file from its `#!` shebang.
fn shebang_language(path: &Path) -> Option<&'static str> {
    let Ok(content) = std::fs::read(path) else {
        return None;
    };
    let head = content.get(..256)?;
    if head.len() < 2 || head[0] != b'#' || head[1] != b'!' {
        return None;
    }
    let line = String::from_utf8_lossy(head).lines().next()?.to_string();
    let lower = line.to_ascii_lowercase();
    if lower.contains("python") {
        Some("python")
    } else if lower.contains("ruby") {
        Some("ruby")
    } else if lower.contains("node") || lower.contains("deno") || lower.contains("bun") {
        Some("javascript")
    } else if lower.contains("bash") || lower.contains("sh") || lower.contains("zsh") {
        Some("bash")
    } else if lower.contains("perl") {
        Some("perl")
    } else if lower.contains("php") {
        Some("php")
    } else if lower.contains("lua") {
        Some("lua")
    } else if lower.contains("elixir") {
        Some("elixir")
    } else if lower.contains("swift") {
        Some("swift")
    } else if lower.contains("go") && line.contains("go run") {
        Some("go")
    } else {
        None
    }
}

/// The language server binary Zed uses by default for this language.
/// (The `--stdio` / `start` flags live in the LSP spawner.)
pub fn lsp_binary_for(lang: &str) -> Option<&'static str> {
    Some(match lang {
        "rust" => "rust-analyzer",
        "go" => "gopls",
        "python" => "basedpyright-langserver",
        "c" | "cpp" => "clangd",
        "csharp" => "omnisharp",
        "javascript" | "typescript" | "tsx" => "typescript-language-server",
        "lua" => "lua-language-server",
        "zig" => "zls",
        "ruby" => "ruby-lsp",
        "java" => "jdtls",
        "php" => "intelephense",
        "bash" => "bash-language-server",
        "yaml" => "yaml-language-server",
        "json" => "vscode-json-language-server",
        "html" => "vscode-html-language-server",
        "css" | "scss" => "vscode-css-language-server",
        "markdown" => "marksman",
        "toml" => "taplo",
        "sql" => "sql-language-server",
        "cmake" => "cmake-language-server",
        "dockerfile" => "dockerfile-language-server-nodejs",
        "graphql" => "graphql-language-service-cli",
        "elixir" => "elixir-ls",
        "swift" => "sourcekit-lsp",
        "scala" => "metals",
        "dart" => "dart",
        "kotlin" => "kotlin-language-server",
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
    // Windows executables come in several flavors; npm global installs
    // create .cmd shims (never .exe), so all of them must be probed.
    const WINDOWS_EXTS: [&str; 3] = [".exe", ".cmd", ".bat"];
    let candidates: Vec<String> = if cfg!(windows) && !WINDOWS_EXTS.iter().any(|e| binary.ends_with(e))
    {
        WINDOWS_EXTS.iter().map(|e| format!("{binary}{e}")).collect()
    } else {
        vec![binary.to_string()]
    };
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                candidates
                    .iter()
                    .any(|name| dir.join(name).is_file())
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_extensions() {
        assert_eq!(language_for(Path::new("main.rs")), Some("rust"));
        assert_eq!(language_for(Path::new("app.tsx")), Some("tsx"));
        assert_eq!(language_for(Path::new("x.cs")), Some("csharp"));
        assert_eq!(language_for(Path::new("Dockerfile")), Some("dockerfile"));
        assert_eq!(language_for(Path::new("CMakeLists.txt")), Some("cmake"));
        assert_eq!(language_for(Path::new("Makefile")), Some("make"));
        assert_eq!(language_for(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn detects_binaries_on_path() {
        // A shell is present on every dev machine.
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(binary_on_path(shell));
        assert!(!binary_on_path("definitely-not-a-binary-xyz"));
    }

    #[test]
    fn maps_lsp_servers() {
        assert_eq!(lsp_binary_for("rust"), Some("rust-analyzer"));
        assert_eq!(lsp_binary_for("typescript"), Some("typescript-language-server"));
        assert_eq!(lsp_binary_for("text"), None);
    }
}
