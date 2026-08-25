//! The explorer's in-memory snapshot of the opened folder. Directory levels
//! are loaded lazily as the user expands nodes.

use std::path::{Path, PathBuf};

/// A node of the explorer tree; children are loaded on first expand.
#[derive(Clone, Debug)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
}

/// Directories / hidden system files never shown in the explorer by default.
const SKIP: &[&str] = &[
    "target",
    "node_modules",
    "dist",
    ".git",
    ".DS_Store",
    "Thumbs.db",
];

/// Read one directory level: dirs first, then files, both case-insensitively alphabetically.
pub fn load_dir(dir: &Path) -> Vec<TreeNode> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<_> = rd.flatten().collect();

    // Sort folders first, then files, both case-insensitively
    entries.sort_by(|a, b| {
        let a_path = a.path();
        let b_path = b.path();
        let a_is_dir = a_path.is_dir();
        let b_is_dir = b_path.is_dir();
        if a_is_dir != b_is_dir {
            return b_is_dir.cmp(&a_is_dir);
        }
        let a_name = a.file_name().to_string_lossy().to_lowercase();
        let b_name = b.file_name().to_string_lossy().to_lowercase();
        a_name.cmp(&b_name)
    });

    for e in entries {
        let path = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if SKIP.iter().any(|s| *s == name) {
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

/// Reloads a directory from disk while preserving the expanded state of subdirectories.
pub fn reload_dir_preserving(dir: &Path, prev_nodes: &[TreeNode]) -> Vec<TreeNode> {
    let mut current = load_dir(dir);
    for node in &mut current {
        if node.is_dir {
            if let Some(prev) = prev_nodes.iter().find(|p| p.path == node.path) {
                if prev.expanded {
                    node.expanded = true;
                    node.children = reload_dir_preserving(&node.path, &prev.children);
                }
            }
        }
    }
    current
}

/// Recursively collapses all open folders.
pub fn collapse_all(nodes: &mut [TreeNode]) {
    for node in nodes {
        node.expanded = false;
        collapse_all(&mut node.children);
    }
}

/// Last path component ("src" for C:\...\project\src).
pub fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}
