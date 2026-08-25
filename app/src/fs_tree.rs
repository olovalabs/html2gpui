//! The explorer's in-memory snapshot of the opened folder. Directory levels
//! are loaded lazily as the user expands nodes.

use std::path::{Path, PathBuf};

/// A node of the explorer tree; children are loaded on first expand.
#[derive(Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

/// Directories never shown in the explorer.
const SKIP: &[&str] = &["target", "node_modules", "dist", ".git"];

/// Read one directory level: dirs first, then files, both alphabetically.
pub fn load_dir(dir: &Path) -> Vec<TreeNode> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| {
        let p = e.path();
        (!p.is_dir(), e.file_name())
    });
    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || SKIP.iter().any(|s| *s == name) {
            continue;
        }
        let is_dir = path.is_dir();
        out.push(TreeNode {
            name,
            path,
            is_dir,
            expanded: false,
            children: Vec::new(),
        });
    }
    out
}

/// Last path component ("src" for C:\...\project\src).
pub fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}
