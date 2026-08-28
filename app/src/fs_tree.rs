//! The explorer's in-memory snapshot of the opened folder. Directory levels
//! are loaded lazily as the user expands nodes.

use std::collections::HashMap;
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

/// One directory entry with every field the sort needs, precomputed once.
///
/// The old implementation called `Path::is_dir()` (a syscall) *inside the
/// sort comparator* — O(n·log n) syscalls — plus allocated two lowercase
/// `String`s per comparison. Materializing the entries once keeps the total
/// probe count at O(n) and makes the comparator allocation-free.
struct Entry {
    name: String,
    /// Case-insensitive sort key: the lowercased name, computed once.
    sort_key: String,
    path: PathBuf,
    is_dir: bool,
}

/// Read one directory level: dirs first, then files, both case-insensitively alphabetically.
pub fn load_dir(dir: &Path) -> Vec<TreeNode> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    let mut entries: Vec<Entry> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if SKIP.iter().any(|s| *s == name) {
            continue;
        }
        // `DirEntry::file_type` is served by the directory's `d_type` field
        // on most platforms (no syscall); it only falls back to `stat()`
        // where the OS leaves the type unknown. Crucially it runs once per
        // entry, never per sort comparison.
        let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        entries.push(Entry {
            sort_key: name.to_lowercase(),
            path: e.path(),
            is_dir,
            name,
        });
    }

    // Dirs first, then files; each group case-insensitively alphabetical.
    // The comparator only reads precomputed fields — no filesystem probes,
    // no per-comparison allocations.
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.sort_key.cmp(&b.sort_key)
    });

    for e in entries {
        out.push(TreeNode {
            name: e.name,
            path: e.path,
            is_dir: e.is_dir,
            expanded: false,
            children: Vec::new(),
        });
    }
    out
}

/// Reloads a directory from disk while preserving the expanded state of subdirectories.
pub fn reload_dir_preserving(dir: &Path, prev_nodes: &[TreeNode]) -> Vec<TreeNode> {
    // Index the previous level by path so state preservation is O(n) instead
    // of the previous linear `find` per node (O(n²) on wide directories).
    let mut prev_by_path: HashMap<&Path, &TreeNode> = HashMap::with_capacity(prev_nodes.len());
    for prev in prev_nodes {
        prev_by_path.insert(prev.path.as_path(), prev);
    }

    let mut current = load_dir(dir);
    for node in &mut current {
        if node.is_dir {
            if let Some(prev) = prev_by_path.get(node.path.as_path()) {
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
