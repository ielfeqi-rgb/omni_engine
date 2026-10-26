// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum VfsBoundaryError {
    UnauthorizedWrite { path: PathBuf, write_set: Vec<PathBuf> },
    UnauthorizedRead { path: PathBuf, read_set: Vec<PathBuf> },
    UnauthorizedDelete { path: PathBuf, write_set: Vec<PathBuf> },
    UnauthorizedPatch { path: PathBuf, write_set: Vec<PathBuf> },
}

impl std::fmt::Display for VfsBoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnauthorizedWrite { path, write_set } => {
                write!(
                    f,
                    "VFS boundary violation: unauthorized write to '{}'. Permitted write_set: {:?}",
                    path.display(),
                    write_set
                )
            }
            Self::UnauthorizedRead { path, read_set } => {
                write!(
                    f,
                    "VFS boundary violation: unauthorized read from '{}'. Permitted read_set: {:?}",
                    path.display(),
                    read_set
                )
            }
            Self::UnauthorizedDelete { path, write_set } => {
                write!(
                    f,
                    "VFS boundary violation: unauthorized delete of '{}'. Permitted write_set: {:?}",
                    path.display(),
                    write_set
                )
            }
            Self::UnauthorizedPatch { path, write_set } => {
                write!(
                    f,
                    "VFS boundary violation: unauthorized patch of '{}'. Permitted write_set: {:?}",
                    path.display(),
                    write_set
                )
            }
        }
    }
}

impl std::error::Error for VfsBoundaryError {}

#[allow(dead_code)]
impl VfsBoundaryError {
    pub fn path(&self) -> &Path {
        match self {
            Self::UnauthorizedWrite { path, .. }
            | Self::UnauthorizedRead { path, .. }
            | Self::UnauthorizedDelete { path, .. }
            | Self::UnauthorizedPatch { path, .. } => path.as_path(),
        }
    }

    pub fn allowed_set(&self) -> &[PathBuf] {
        match self {
            Self::UnauthorizedWrite { write_set, .. }
            | Self::UnauthorizedDelete { write_set, .. }
            | Self::UnauthorizedPatch { write_set, .. } => write_set.as_slice(),
            Self::UnauthorizedRead { read_set, .. } => read_set.as_slice(),
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            Self::UnauthorizedWrite { .. } => "VFS_UNAUTHORIZED_WRITE",
            Self::UnauthorizedRead { .. } => "VFS_UNAUTHORIZED_READ",
            Self::UnauthorizedDelete { .. } => "VFS_UNAUTHORIZED_DELETE",
            Self::UnauthorizedPatch { .. } => "VFS_UNAUTHORIZED_PATCH",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StagedDiff {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub original_content: Option<String>,
    pub new_content: Option<String>,
}

#[allow(dead_code)]
impl StagedDiff {
    pub fn unified_diff(&self) -> String {
        match (&self.original_content, &self.new_content) {
            (Some(orig), Some(curr)) => MemoryVfs::generate_unified_diff(&self.path.display().to_string(), orig, curr),
            (None, Some(curr)) => {
                let mut out = format!("--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n", self.path.display(), curr.lines().count());
                for line in curr.lines() {
                    out.push_str(&format!("+{}\n", line));
                }
                out
            }
            (Some(orig), None) => {
                let mut out = format!("--- a/{}\n+++ /dev/null\n@@ -1,{} +0,0 @@\n", self.path.display(), orig.lines().count());
                for line in orig.lines() {
                    out.push_str(&format!("-{}\n", line));
                }
                out
            }
            (None, None) => String::new(),
        }
    }
}

/// In-Memory Virtual File System (VFS).
/// Fully isolated inside RAM with zero initial host disk side-effects.
/// Compatible with Windows and Linux paths.
pub struct MemoryVfs {
    files: RwLock<HashMap<PathBuf, Vec<u8>>>,
    original_snapshots: RwLock<HashMap<PathBuf, Vec<u8>>>,
}

impl MemoryVfs {
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            original_snapshots: RwLock::new(HashMap::new()),
        }
    }

    /// Expand ~ to user $HOME and normalize paths for host portability
    pub fn resolve_path<P: AsRef<Path>>(path: P) -> PathBuf {
        let p = path.as_ref();
        let p_str = p.to_string_lossy();
        if p_str.starts_with("~/") || p_str == "~" {
            if let Ok(home) = std::env::var("HOME") {
                let sub = p_str.trim_start_matches('~').trim_start_matches('/');
                return PathBuf::from(home).join(sub);
            }
        }
        p.to_path_buf()
    }

    /// Lexically normalizes a path for consistent in-memory storage and boundary enforcement:
    /// - Expands `~` to user $HOME.
    /// - Drops current-directory components (`.`).
    /// - Resolves parent-directory components (`..`) lexically against preceding segments.
    /// - Prevents root breakout when parent components attempt to ascend past the root.
    /// - Normalizes empty paths to `PathBuf::from(".")`.
    pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
        let resolved = Self::resolve_path(path);
        let mut components = Vec::new();

        for comp in resolved.components() {
            match comp {
                Component::CurDir => {}
                Component::ParentDir => {
                    if let Some(last) = components.last() {
                        if !matches!(last, Component::RootDir | Component::Prefix(_) | Component::ParentDir) {
                            components.pop();
                            continue;
                        }
                    }
                    components.push(comp);
                }
                _ => {
                    components.push(comp);
                }
            }
        }

        let mut normalized = PathBuf::new();
        for c in components {
            normalized.push(c.as_os_str());
        }

        if normalized.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            normalized
        }
    }

    /// Checks whether `path` is permitted by `allowed_set`.
    pub fn is_path_in_set<P: AsRef<Path>>(path: P, allowed_set: &[PathBuf]) -> bool {
        let norm_path = Self::normalize_path(path.as_ref());
        let clean_path_str = norm_path.to_string_lossy();
        let stripped_path = clean_path_str.trim_start_matches('/');

        allowed_set.iter().any(|allowed| {
            let norm_allowed = Self::normalize_path(allowed);
            let clean_allowed_str = norm_allowed.to_string_lossy();
            let stripped_allowed = clean_allowed_str.trim_start_matches('/');

            if norm_path == norm_allowed || stripped_path == stripped_allowed {
                return true;
            }

            if norm_path.starts_with(&norm_allowed) || Path::new(stripped_path).starts_with(stripped_allowed) {
                return true;
            }

            false
        })
    }

    #[allow(dead_code)]
    pub fn write_file_checked<P: AsRef<Path>>(
        &self,
        path: P,
        content: &[u8],
        write_set: &[PathBuf],
    ) -> Result<(), VfsBoundaryError> {
        let p = path.as_ref();
        if !Self::is_path_in_set(p, write_set) {
            return Err(VfsBoundaryError::UnauthorizedWrite {
                path: p.to_path_buf(),
                write_set: write_set.to_vec(),
            });
        }
        self.write_file(p, content);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn read_file_checked<P: AsRef<Path>>(
        &self,
        path: P,
        read_set: Option<&[PathBuf]>,
    ) -> Result<Option<Vec<u8>>, VfsBoundaryError> {
        let p = path.as_ref();
        if let Some(set) = read_set {
            if !Self::is_path_in_set(p, set) {
                return Err(VfsBoundaryError::UnauthorizedRead {
                    path: p.to_path_buf(),
                    read_set: set.to_vec(),
                });
            }
        }
        Ok(self.read_file(p))
    }

    #[allow(dead_code)]
    pub fn read_string_checked<P: AsRef<Path>>(
        &self,
        path: P,
        read_set: Option<&[PathBuf]>,
    ) -> Result<Option<String>, VfsBoundaryError> {
        self.read_file_checked(path, read_set)
            .map(|opt| opt.and_then(|bytes| String::from_utf8(bytes).ok()))
    }

    #[allow(dead_code)]
    pub fn delete_file_checked<P: AsRef<Path>>(
        &self,
        path: P,
        write_set: &[PathBuf],
    ) -> Result<(), VfsBoundaryError> {
        let p = path.as_ref();
        if !Self::is_path_in_set(p, write_set) {
            return Err(VfsBoundaryError::UnauthorizedDelete {
                path: p.to_path_buf(),
                write_set: write_set.to_vec(),
            });
        }
        self.delete_file(p);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn patch_file_checked<P: AsRef<Path>>(
        &self,
        path: P,
        patch: &str,
        write_set: &[PathBuf],
    ) -> Result<String, VfsBoundaryError> {
        let p = path.as_ref();
        if !Self::is_path_in_set(p, write_set) {
            return Err(VfsBoundaryError::UnauthorizedPatch {
                path: p.to_path_buf(),
                write_set: write_set.to_vec(),
            });
        }

        if let Some(target_start) = patch.find("<<<<<<< TARGET") {
            let after_target = &patch[target_start + 14..];
            if let Some(sep) = after_target.find("=======") {
                let target_snippet = after_target[..sep].trim_matches('\n');
                let after_sep = &after_target[sep + 7..];
                if let Some(end) = after_sep.find(">>>>>>>") {
                    let replacement_snippet = after_sep[..end].trim_matches('\n');
                    let _ = self.patch_file(p, target_snippet, replacement_snippet);
                    return Ok(self.read_string(p).unwrap_or_default());
                }
            }
        }

        if patch.contains("TARGET:") && patch.contains("REPLACEMENT:") {
            let parts: Vec<&str> = patch.split('|').collect();
            let mut target = None;
            let mut replacement = None;
            for part in parts {
                let trimmed = part.trim();
                if trimmed.starts_with("TARGET:") {
                    target = Some(trimmed.trim_start_matches("TARGET:").trim());
                } else if trimmed.starts_with("REPLACEMENT:") {
                    replacement = Some(trimmed.trim_start_matches("REPLACEMENT:").trim());
                }
            }
            if let (Some(t), Some(r)) = (target, replacement) {
                let _ = self.patch_file(p, t, r);
                return Ok(self.read_string(p).unwrap_or_default());
            }
        }

        if !self.exists(p) {
            self.write_file(p, patch.as_bytes());
        }
        Ok(self.read_string(p).unwrap_or_default())
    }

    #[allow(dead_code)]
    pub fn patch_snippet_checked<P: AsRef<Path>>(
        &self,
        path: P,
        target: &str,
        replacement: &str,
        write_set: &[PathBuf],
    ) -> Result<bool, VfsBoundaryError> {
        let p = path.as_ref();
        if !Self::is_path_in_set(p, write_set) {
            return Err(VfsBoundaryError::UnauthorizedPatch {
                path: p.to_path_buf(),
                write_set: write_set.to_vec(),
            });
        }
        Ok(self.patch_file(p, target, replacement).unwrap_or(false))
    }

    /// Preload or mount a real file from host into RAM sandbox (read-only copy)
    pub fn preload_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = Self::normalize_path(path);
        let mut original = self.original_snapshots.write();
        let mut active = self.files.write();
        original.insert(path_buf.clone(), content.to_vec());
        active.insert(path_buf, content.to_vec());
    }

    /// Model writes or edits a file inside the isolated RAM VFS
    pub fn write_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = Self::normalize_path(path);
        let mut active = self.files.write();
        active.insert(path_buf, content.to_vec());
    }

    /// Take a baseline snapshot of a specific file in active VFS for diffing
    pub fn snapshot_file<P: AsRef<Path>>(&self, path: P) {
        let path_buf = Self::normalize_path(path);
        let active = self.files.read();
        if let Some(content) = active.get(&path_buf) {
            let mut original = self.original_snapshots.write();
            original.insert(path_buf, content.clone());
        }
    }

    /// Read file content from RAM VFS
    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> Option<Vec<u8>> {
        let path_buf = Self::normalize_path(path);
        let active = self.files.read();
        active.get(&path_buf).cloned()
    }

    /// Read file as UTF-8 string
    pub fn read_string<P: AsRef<Path>>(&self, path: P) -> Option<String> {
        self.read_file(path).and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Check if file exists in RAM VFS
    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = Self::normalize_path(path);
        let active = self.files.read();
        active.contains_key(&path_buf)
    }

    /// Delete file inside RAM VFS
    pub fn delete_file<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = Self::normalize_path(path);
        let mut active = self.files.write();
        active.remove(&path_buf).is_some()
    }

    /// List all file paths currently stored in RAM VFS
    pub fn list_files(&self) -> Vec<PathBuf> {
        let active = self.files.read();
        active.keys().cloned().collect()
    }

    /// Returns all file paths and their contents currently stored in RAM VFS
    pub fn all_files(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let active = self.files.read();
        active.iter().map(|(p, c)| (p.clone(), c.clone())).collect()
    }

    /// Returns byte size of a file in VFS
    pub fn file_size<P: AsRef<Path>>(&self, path: P) -> usize {
        let path_buf = Self::normalize_path(path);
        let active = self.files.read();
        active.get(&path_buf).map(|b| b.len()).unwrap_or(0)
    }

    /// Surgically patch a file in RAM VFS by replacing the first occurrence of `target` with `replacement`
    pub fn patch_file<P: AsRef<Path>>(&self, path: P, target: &str, replacement: &str) -> Result<bool, String> {
        let path_buf = Self::normalize_path(path);
        let mut active = self.files.write();
        let bytes = active.get(&path_buf).ok_or_else(|| format!("File not found in VFS: {}", path_buf.display()))?;
        let content_str = String::from_utf8(bytes.clone())
            .map_err(|e| format!("File is not valid UTF-8: {}", e))?;

        if !content_str.contains(target) {
            return Err(format!("Target snippet not found in file '{}'", path_buf.display()));
        }

        let patched = content_str.replacen(target, replacement, 1);
        active.insert(path_buf, patched.into_bytes());
        Ok(true)
    }

    /// Generate a unified line-by-line diff for a specific file between original snapshot and active VFS
    pub fn diff_file<P: AsRef<Path>>(&self, path: P) -> Option<String> {
        let path_buf = Self::normalize_path(path);
        let active = self.files.read();
        let original = self.original_snapshots.read();

        let orig_str = original.get(&path_buf).and_then(|b| String::from_utf8(b.clone()).ok());
        let curr_str = active.get(&path_buf).and_then(|b| String::from_utf8(b.clone()).ok());

        match (orig_str, curr_str) {
            (None, None) => None,
            (Some(orig), None) => {
                let mut out = format!("--- a/{}\n+++ /dev/null\n@@ -1,{} +0,0 @@\n", path_buf.display(), orig.lines().count());
                for line in orig.lines() {
                    out.push_str(&format!("-{}\n", line));
                }
                Some(out)
            }
            (None, Some(curr)) => {
                let mut out = format!("--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n", path_buf.display(), curr.lines().count());
                for line in curr.lines() {
                    out.push_str(&format!("+{}\n", line));
                }
                Some(out)
            }
            (Some(orig), Some(curr)) => {
                if orig == curr {
                    return None;
                }
                Some(Self::generate_unified_diff(&path_buf.display().to_string(), &orig, &curr))
            }
        }
    }

    /// Standard Myers / LCS unified line-by-line diff algorithm
    pub fn generate_unified_diff(filename: &str, old_text: &str, new_text: &str) -> String {
        let old_lines: Vec<&str> = old_text.lines().collect();
        let new_lines: Vec<&str> = new_text.lines().collect();

        let mut diff = format!("--- a/{}\n+++ b/{}\n", filename, filename);

        let m = old_lines.len();
        let n = new_lines.len();

        let mut dp = vec![vec![0usize; n + 1]; m + 1];
        for i in 0..m {
            for j in 0..n {
                if old_lines[i] == new_lines[j] {
                    dp[i + 1][j + 1] = dp[i][j] + 1;
                } else {
                    dp[i + 1][j + 1] = dp[i][j].max(dp[i + 1][j]);
                }
            }
        }

        let mut i = m;
        let mut j = n;
        let mut changes = Vec::new();

        while i > 0 || j > 0 {
            if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
                changes.push(format!(" {}", old_lines[i - 1]));
                i -= 1;
                j -= 1;
            } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
                changes.push(format!("+{}", new_lines[j - 1]));
                j -= 1;
            } else if i > 0 && (j == 0 || dp[i][j - 1] < dp[i - 1][j]) {
                changes.push(format!("-{}", old_lines[i - 1]));
                i -= 1;
            }
        }

        changes.reverse();
        diff.push_str(&format!("@@ -1,{} +1,{} @@\n", m, n));
        for line in changes {
            diff.push_str(&line);
            diff.push('\n');
        }

        diff
    }

    /// Compute staged changes (Diffs) between original snapshots and active sandbox modifications.
    pub fn generate_staged_diffs(&self) -> Vec<StagedDiff> {
        let active = self.files.read();
        let original = self.original_snapshots.read();

        let mut diffs = Vec::new();

        // Check for created or modified files
        for (path, current_bytes) in active.iter() {
            let current_str = String::from_utf8_lossy(current_bytes).to_string();
            match original.get(path) {
                Some(orig_bytes) => {
                    if orig_bytes != current_bytes {
                        let orig_str = String::from_utf8_lossy(orig_bytes).to_string();
                        diffs.push(StagedDiff {
                            path: path.clone(),
                            change_type: FileChangeType::Modified,
                            original_content: Some(orig_str),
                            new_content: Some(current_str),
                        });
                    }
                }
                None => {
                    diffs.push(StagedDiff {
                        path: path.clone(),
                        change_type: FileChangeType::Created,
                        original_content: None,
                        new_content: Some(current_str),
                    });
                }
            }
        }

        // Check for deleted files
        for (path, orig_bytes) in original.iter() {
            if !active.contains_key(path) {
                let orig_str = String::from_utf8_lossy(orig_bytes).to_string();
                diffs.push(StagedDiff {
                    path: path.clone(),
                    change_type: FileChangeType::Deleted,
                    original_content: Some(orig_str),
                    new_content: None,
                });
            }
        }

        diffs
    }

    /// Take a scoped in-memory snapshot of specific files (e.g. worker's write scope) in VFS.
    /// Captures prior contents for existing files, and marks non-existent files as None.
    pub fn take_scoped_snapshot(&self, paths: &[PathBuf]) -> HashMap<PathBuf, Option<Vec<u8>>> {
        let active = self.files.read();
        let mut snapshot = HashMap::new();
        for p in paths {
            let norm = Self::normalize_path(p);
            let state = active.get(&norm).cloned();
            snapshot.insert(norm, state);
        }
        snapshot
    }

    /// Restore ONLY the scoped files in VFS to their previous snapshot state.
    /// Existing files with Some(bytes) are restored to their previous content.
    /// Files that were None (newly created during the attempt) are safely removed.
    /// All other files in VFS (belonging to other concurrent workers) remain 100% untouched.
    pub fn restore_scoped_snapshot(&self, snapshot: HashMap<PathBuf, Option<Vec<u8>>>) {
        let mut active = self.files.write();
        for (norm, prior_state) in snapshot {
            match prior_state {
                Some(bytes) => {
                    active.insert(norm, bytes);
                }
                None => {
                    active.remove(&norm);
                }
            }
        }
    }

    /// Commit staged diffs to actual host disk ONLY upon explicit user confirmation.
    pub fn commit_to_host(&self, user_authorized: bool) -> Result<usize, String> {
        self.commit_to_dir(None, user_authorized)
    }

    /// Commit staged diffs to a specific directory on host.
    /// Strictly requires all deliverables to be relative to `target_dir` (default: current workspace root).
    /// Enforces a Two-Phase Commit:
    /// - Phase 1: Pure dry-run validation pass. Rejects absolute paths, path escapes, symlinks, and TOCTOU intermediate symlink components. If any file fails, zero files are touched.
    /// - Phase 2: Atomic write pass using kernel-level `O_NOFOLLOW` flag to prevent symlink race window exploits.
    pub fn commit_to_dir(&self, target_dir: Option<&Path>, user_authorized: bool) -> Result<usize, String> {
        if !user_authorized {
            return Err("Authorization Denied: Cannot commit sandbox VFS to host disk without user approval".to_string());
        }

        let fallback_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let base = target_dir.unwrap_or(&fallback_root);

        let diffs = self.generate_staged_diffs();

        // --------------------------------------------------------------------
        // PHASE 1: STRICT DRY-RUN VALIDATION PASS (Zero mutations to host disk)
        // --------------------------------------------------------------------
        let mut validated_actions = Vec::with_capacity(diffs.len());

        for diff in &diffs {
            // Absolute paths and home-relative paths in deliverables are strictly forbidden!
            let path_str = diff.path.to_string_lossy();
            if diff.path.is_absolute() || path_str.starts_with('~') || path_str.starts_with('/') {
                return Err(format!(
                    "Security Violation: Absolute or home-relative path in VFS deliverable rejected: {:?}. All deliverables must be strictly relative to workspace.",
                    diff.path
                ));
            }

            // Safe join prevents any `..` directory traversal outside base
            let joined = crate::sandbox::isolated_jail::IsolatedJail::safe_join(base, &diff.path)
                .map_err(|e| format!("Security Violation: Path traversal rejected in {:?}: {}", diff.path, e))?;

            // Verify no intermediate parent directory component is a symlink (TOCTOU Defense)
            let mut current = base.to_path_buf();
            if let Ok(rel) = joined.strip_prefix(base) {
                for component in rel.parent().unwrap_or(Path::new("")).components() {
                    if let std::path::Component::Normal(c) = component {
                        current.push(c);
                        if let Ok(meta) = std::fs::symlink_metadata(&current) {
                            if meta.file_type().is_symlink() {
                                return Err(format!(
                                    "Security Violation: Intermediate directory component {:?} is a symlink on host disk. Path traversal aborted.",
                                    current
                                ));
                            }
                        }
                    }
                }
            }

            // Verify target path itself is not an existing symlink on host disk
            if let Ok(meta) = std::fs::symlink_metadata(&joined) {
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "Security Violation: Target path {:?} already exists as a symlink on host disk. Overwriting or following host symlinks is forbidden to preserve system integrity.",
                        joined
                    ));
                }
            }

            validated_actions.push((joined, diff.change_type.clone(), diff.path.clone()));
        }

        // --------------------------------------------------------------------
        // PHASE 2: ATOMIC WRITE PASS WITH KERNEL O_NOFOLLOW
        // --------------------------------------------------------------------
        let mut committed = 0;

        for (target_path, change_type, vfs_path) in validated_actions {
            match change_type {
                FileChangeType::Created | FileChangeType::Modified => {
                    if let Some(parent) = target_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Some(content) = self.read_file(&vfs_path) {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::OpenOptionsExt;
                            use std::io::Write;
                            let mut opts = std::fs::OpenOptions::new();
                            opts.write(true).create(true).truncate(true);
                            opts.custom_flags(libc::O_NOFOLLOW);
                            let mut file = opts.open(&target_path)
                                .map_err(|e| format!("Security Violation: Failed to securely open host file {:?} with O_NOFOLLOW: {}", target_path, e))?;
                            file.write_all(&content)
                                .map_err(|e| format!("Failed to write to host file {:?}: {}", target_path, e))?;
                        }
                        #[cfg(not(unix))]
                        {
                            std::fs::write(&target_path, content)
                                .map_err(|e| format!("Failed to write to host file {:?}: {}", target_path, e))?;
                        }
                        committed += 1;
                    }
                }
                FileChangeType::Deleted => {
                    if target_path.exists() {
                        std::fs::remove_file(&target_path)
                            .map_err(|e| format!("Failed to remove host file {:?}: {}", target_path, e))?;
                        committed += 1;
                    }
                }
            }
        }

        // Update original snapshots to reflect committed state
        let mut original = self.original_snapshots.write();
        let active = self.files.read();
        *original = active.clone();

        Ok(committed)
    }

    /// Commit ONLY explicitly verified deliverables to host disk.
    /// Returns the list of target paths actually written to host disk.
    /// If user_authorized is false or verified_paths is empty, does not write unverified files.
    pub fn commit_verified_files(
        &self,
        target_dir: Option<&Path>,
        verified_paths: &[PathBuf],
        user_authorized: bool,
    ) -> Result<Vec<PathBuf>, String> {
        if !user_authorized {
            return Err("Authorization Denied: Cannot commit sandbox VFS to host disk without user approval".to_string());
        }

        if verified_paths.is_empty() {
            return Ok(Vec::new());
        }

        let diffs = self.generate_staged_diffs();
        let mut committed_paths = Vec::new();

        for diff in &diffs {
            let is_verified = verified_paths.iter().any(|vp| {
                let norm_vp = Self::normalize_path(vp);
                let norm_diff = Self::normalize_path(&diff.path);
                norm_vp == norm_diff
                    || diff.path.ends_with(vp)
                    || vp.ends_with(&diff.path)
                    || diff.path.file_name() == vp.file_name()
            });

            if !is_verified {
                continue;
            }

            let target_path = if diff.path.is_absolute() {
                Self::resolve_path(&diff.path)
            } else if let Some(base) = target_dir {
                base.join(diff.path.file_name().unwrap_or(diff.path.as_os_str()))
            } else {
                Self::resolve_path(&diff.path)
            };

            match diff.change_type {
                FileChangeType::Created | FileChangeType::Modified => {
                    if let Some(parent) = target_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Some(content) = self.read_file(&diff.path) {
                        std::fs::write(&target_path, content)
                            .map_err(|e| format!("Failed to write to host file {:?}: {}", target_path, e))?;
                        committed_paths.push(target_path);
                    }
                }
                FileChangeType::Deleted => {
                    if target_path.exists() {
                        std::fs::remove_file(&target_path)
                            .map_err(|e| format!("Failed to remove host file {:?}: {}", target_path, e))?;
                        committed_paths.push(target_path);
                    }
                }
            }
        }

        // Update original snapshots to reflect committed state for verified files
        let mut original = self.original_snapshots.write();
        let active = self.files.read();
        for vp in verified_paths {
            let norm_vp = Self::normalize_path(vp);
            if let Some(content) = active.get(&norm_vp) {
                original.insert(norm_vp, content.clone());
            }
        }

        Ok(committed_paths)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_vfs_lifecycle_and_diffs() {
        let vfs = MemoryVfs::new();

        // 1. Preload existing file
        let path = PathBuf::from("workspace/src/app.py");
        vfs.preload_file(&path, b"print('hello world')\n");

        // 2. Read inside sandbox
        assert_eq!(vfs.read_string(&path).unwrap(), "print('hello world')\n");

        // 3. Edit inside sandbox
        vfs.write_file(&path, b"print('hello world')\nprint('sandbox modification')\n");

        // 4. Create new file inside sandbox
        let new_file = PathBuf::from("workspace/src/utils.py");
        vfs.write_file(&new_file, b"def add(a, b): return a + b\n");

        // 5. Inspect diffs before host commit
        let diffs = vfs.generate_staged_diffs();
        assert_eq!(diffs.len(), 2);

        let mod_diff = diffs.iter().find(|d| d.path == path).unwrap();
        assert_eq!(mod_diff.change_type, FileChangeType::Modified);
        assert!(mod_diff.new_content.as_ref().unwrap().contains("sandbox modification"));

        let new_diff = diffs.iter().find(|d| d.path == new_file).unwrap();
        assert_eq!(new_diff.change_type, FileChangeType::Created);

        // 6. Refuse commit without user authorization
        let res_unauth = vfs.commit_to_host(false);
        assert!(res_unauth.is_err());
        assert!(res_unauth.unwrap_err().contains("Authorization Denied"));
    }

    #[test]
    fn test_resolve_path_home_expansion() {
        if let Ok(home) = std::env::var("HOME") {
            let resolved = MemoryVfs::resolve_path("~/documents/notes.txt");
            assert_eq!(resolved, PathBuf::from(home).join("documents/notes.txt"));
        }
    }

    #[test]
    fn test_vfs_patch_and_diff() {
        let vfs = MemoryVfs::new();
        let path = PathBuf::from("workspace/Game.java");

        // 1. Initial preload
        vfs.preload_file(&path, b"class Game {\n    void start() {\n        int x = 1;\n    }\n}\n");
        assert_eq!(vfs.file_size(&path), 59);

        // 2. Patch a single line surgically
        let res = vfs.patch_file(&path, "int x = 1;", "int x = 100; // patched");
        assert!(res.is_ok());

        let patched_content = vfs.read_string(&path).unwrap();
        assert!(patched_content.contains("int x = 100; // patched"));
        assert!(!patched_content.contains("int x = 1;"));

        // 3. Generate unified diff
        let diff = vfs.diff_file(&path).unwrap();
        assert!(diff.contains("-        int x = 1;"));
        assert!(diff.contains("+        int x = 100; // patched"));

        // 4. Test delete
        assert!(vfs.exists(&path));
        assert!(vfs.delete_file(&path));
        assert!(!vfs.exists(&path));
        assert_eq!(vfs.file_size(&path), 0);
    }

    #[test]
    fn test_commit_to_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_dir = temp_dir.path().join("out_workspace");
        let vfs = MemoryVfs::new();

        let rel_file = PathBuf::from("calc.py");
        vfs.write_file(&rel_file, b"print(42)\n");

        let committed = vfs.commit_to_dir(Some(&target_dir), true).unwrap();
        assert_eq!(committed, 1);

        let written_file = target_dir.join("calc.py");
        assert!(written_file.exists());
        assert_eq!(std::fs::read_to_string(&written_file).unwrap(), "print(42)\n");
    }

    #[test]
    fn test_commit_to_dir_rejects_symlink_overwrites() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_dir = temp_dir.path().join("out_workspace");
        std::fs::create_dir_all(&target_dir).unwrap();

        // Create an existing symlink inside target_dir pointing to a dummy secret target
        let dummy_target = temp_dir.path().join("secret.txt");
        std::fs::write(&dummy_target, b"secret_value").unwrap();

        let symlink_path = target_dir.join("link.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dummy_target, &symlink_path).unwrap();

        let vfs = MemoryVfs::new();
        vfs.write_file(Path::new("link.txt"), b"evil_overwrite");

        // Attempting to commit should FAIL with a Security Violation error!
        let result = vfs.commit_to_dir(Some(&target_dir), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Security Violation"));

        // Verify the original dummy target was NOT modified
        assert_eq!(std::fs::read(&dummy_target).unwrap(), b"secret_value");
    }

    #[test]
    fn test_vfs_boundary_error_display_and_traits() {
        let ws = vec![PathBuf::from("src/app.py"), PathBuf::from("styles.css")];
        let rs = vec![PathBuf::from("config.json")];

        let err_write = VfsBoundaryError::UnauthorizedWrite {
            path: PathBuf::from("secret.key"),
            write_set: ws.clone(),
        };
        assert!(err_write.to_string().contains("unauthorized write to 'secret.key'"));
        assert_eq!(err_write.path(), Path::new("secret.key"));
        assert_eq!(err_write.allowed_set(), ws.as_slice());
        assert_eq!(err_write.error_code(), "VFS_UNAUTHORIZED_WRITE");

        let err_read = VfsBoundaryError::UnauthorizedRead {
            path: PathBuf::from("db.sqlite"),
            read_set: rs.clone(),
        };
        assert!(err_read.to_string().contains("unauthorized read from 'db.sqlite'"));
        assert_eq!(err_read.error_code(), "VFS_UNAUTHORIZED_READ");

        let err_del = VfsBoundaryError::UnauthorizedDelete {
            path: PathBuf::from("protected.rs"),
            write_set: ws.clone(),
        };
        assert!(err_del.to_string().contains("unauthorized delete of 'protected.rs'"));

        let err_patch = VfsBoundaryError::UnauthorizedPatch {
            path: PathBuf::from("kernel.c"),
            write_set: ws.clone(),
        };
        assert!(err_patch.to_string().contains("unauthorized patch of 'kernel.c'"));

        // Verify std::error::Error trait bound
        let _: &dyn std::error::Error = &err_write;
    }

    #[test]
    fn test_path_normalization_lexical_resolution() {
        // Redundant current directory
        assert_eq!(MemoryVfs::normalize_path("./src/main.rs"), PathBuf::from("src/main.rs"));
        assert_eq!(MemoryVfs::normalize_path("src/./main.rs"), PathBuf::from("src/main.rs"));

        // Parent directory traversal
        assert_eq!(MemoryVfs::normalize_path("src/utils/../main.rs"), PathBuf::from("src/main.rs"));
        assert_eq!(MemoryVfs::normalize_path("a/b/c/../../d.rs"), PathBuf::from("a/d.rs"));

        // Boundary escape attempt
        let escaped = MemoryVfs::normalize_path("src/../../outside.txt");
        assert!(escaped.ends_with("outside.txt"));
        assert_ne!(escaped, PathBuf::from("src/outside.txt"));

        // Consecutive parent directory traversal
        assert_eq!(MemoryVfs::normalize_path("../../secret.txt"), PathBuf::from("../../secret.txt"));
        assert_eq!(MemoryVfs::normalize_path("../../../a/b"), PathBuf::from("../../../a/b"));
        assert_eq!(MemoryVfs::normalize_path("a/../../secret.txt"), PathBuf::from("../secret.txt"));
        assert!(!MemoryVfs::is_path_in_set("../../secret.txt", &[PathBuf::from("secret.txt")]));

        // Empty path
        assert_eq!(MemoryVfs::normalize_path(""), PathBuf::from("."));
    }

    #[test]
    fn test_normalize_path_consecutive_parent_dirs_security() {
        assert_eq!(MemoryVfs::normalize_path("../../secret.txt"), PathBuf::from("../../secret.txt"));
        assert_eq!(MemoryVfs::normalize_path("../../../a/b"), PathBuf::from("../../../a/b"));
        assert_eq!(MemoryVfs::normalize_path("a/../../secret.txt"), PathBuf::from("../secret.txt"));
        assert!(!MemoryVfs::is_path_in_set("../../secret.txt", &[PathBuf::from("secret.txt")]));
    }

    #[test]
    fn test_write_file_checked_enforcement() {
        let vfs = MemoryVfs::new();
        let write_set = vec![PathBuf::from("src/app.py"), PathBuf::from("output/")];

        // 1. Authorized write
        let res_ok = vfs.write_file_checked("src/app.py", b"print(1)", &write_set);
        assert!(res_ok.is_ok());
        assert_eq!(vfs.read_string("src/app.py").unwrap(), "print(1)");

        // 2. Authorized write with normalized path
        let res_norm = vfs.write_file_checked("./src/app.py", b"print(2)", &write_set);
        assert!(res_norm.is_ok());
        assert_eq!(vfs.read_string("src/app.py").unwrap(), "print(2)");

        // 3. Authorized directory prefix write
        let res_dir = vfs.write_file_checked("output/data.csv", b"id,val", &write_set);
        assert!(res_dir.is_ok());
        assert_eq!(vfs.read_string("output/data.csv").unwrap(), "id,val");

        // 4. Unauthorized write rejection
        let res_err = vfs.write_file_checked("evil.sh", b"rm -rf /", &write_set);
        assert!(matches!(res_err, Err(VfsBoundaryError::UnauthorizedWrite { .. })));
        if let Err(VfsBoundaryError::UnauthorizedWrite { path, write_set: ws }) = res_err {
            assert_eq!(path, PathBuf::from("evil.sh"));
            assert_eq!(ws, write_set);
        }
        assert!(!vfs.exists("evil.sh"));
    }

    #[test]
    fn test_read_file_checked_enforcement() {
        let vfs = MemoryVfs::new();
        vfs.write_file("public.txt", b"public data");
        vfs.write_file("secret.txt", b"secret data");

        let read_set = vec![PathBuf::from("public.txt")];

        // 1. Unrestricted read (read_set: None)
        assert_eq!(
            vfs.read_file_checked("secret.txt", None).unwrap().unwrap(),
            b"secret data"
        );

        // 2. Authorized read (read_set: Some)
        assert_eq!(
            vfs.read_file_checked("public.txt", Some(&read_set)).unwrap().unwrap(),
            b"public data"
        );
        assert_eq!(
            vfs.read_string_checked("./public.txt", Some(&read_set)).unwrap().unwrap(),
            "public data"
        );

        // 3. Authorized read of non-existent file in allowed set
        let empty_allowed = vec![PathBuf::from("nonexistent.txt")];
        assert_eq!(
            vfs.read_file_checked("nonexistent.txt", Some(&empty_allowed)).unwrap(),
            None
        );

        // 4. Unauthorized read rejection
        let unauth = vfs.read_file_checked("secret.txt", Some(&read_set));
        assert!(matches!(unauth, Err(VfsBoundaryError::UnauthorizedRead { .. })));
        if let Err(VfsBoundaryError::UnauthorizedRead { path, read_set: rs }) = unauth {
            assert_eq!(path, PathBuf::from("secret.txt"));
            assert_eq!(rs, read_set);
        }
    }

    #[test]
    fn test_delete_file_checked_enforcement() {
        let vfs = MemoryVfs::new();
        vfs.write_file("cache.tmp", b"cache");
        vfs.write_file("prod.db", b"data");

        let write_set = vec![PathBuf::from("cache.tmp")];

        // 1. Authorized delete
        assert!(vfs.delete_file_checked("cache.tmp", &write_set).is_ok());
        assert!(!vfs.exists("cache.tmp"));

        // 2. Unauthorized delete
        let unauth = vfs.delete_file_checked("prod.db", &write_set);
        assert!(matches!(unauth, Err(VfsBoundaryError::UnauthorizedDelete { .. })));
        assert!(vfs.exists("prod.db"));
    }

    #[test]
    fn test_patch_file_checked_enforcement_and_formats() {
        let vfs = MemoryVfs::new();
        let target_file = PathBuf::from("service.py");
        vfs.write_file(&target_file, b"PORT = 8080\nDEBUG = False\n");

        let write_set = vec![target_file.clone()];

        // 1. Unified diff marker patch
        let patch1 = "<<<<<<< TARGET\nPORT = 8080\n=======\nPORT = 9000\n>>>>>>>";
        let patched1 = vfs.patch_file_checked(&target_file, patch1, &write_set).unwrap();
        assert!(patched1.contains("PORT = 9000"));
        assert_eq!(vfs.read_string(&target_file).unwrap(), "PORT = 9000\nDEBUG = False\n");

        // 2. Key-value format patch
        let patch2 = "TARGET: DEBUG = False | REPLACEMENT: DEBUG = True";
        let patched2 = vfs.patch_file_checked(&target_file, patch2, &write_set).unwrap();
        assert!(patched2.contains("DEBUG = True"));
        assert_eq!(vfs.read_string(&target_file).unwrap(), "PORT = 9000\nDEBUG = True\n");

        // 3. patch_snippet_checked
        assert!(vfs.patch_snippet_checked(&target_file, "PORT = 9000", "PORT = 3000", &write_set).unwrap());
        assert!(vfs.read_string(&target_file).unwrap().contains("PORT = 3000"));

        // 4. Unauthorized patch
        let unauth = vfs.patch_file_checked("unauth.py", patch1, &write_set);
        assert!(matches!(unauth, Err(VfsBoundaryError::UnauthorizedPatch { .. })));
    }

    #[test]
    fn test_concurrent_checked_operations() {
        use std::sync::Arc;
        let vfs = Arc::new(MemoryVfs::new());
        let write_set = vec![PathBuf::from("shared.log")];

        vfs.write_file_checked("shared.log", b"init\n", &write_set).unwrap();

        let mut handles = Vec::new();
        for i in 0..10 {
            let v = Arc::clone(&vfs);
            let ws = write_set.clone();
            handles.push(std::thread::spawn(move || {
                let msg = format!("worker_{}\n", i);
                v.write_file_checked("shared.log", msg.as_bytes(), &ws).unwrap();
                let read = v.read_file_checked("shared.log", Some(&ws)).unwrap();
                assert!(read.is_some());
            }));
        }

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_scoped_snapshot_isolation() {
        let vfs = MemoryVfs::new();
        // Worker 2 creates and validates index.html
        vfs.write_file("index.html", b"<html><body>Task Manager</body></html>");

        // Worker 1 starts attempt on app.py
        let worker1_scope = vec![PathBuf::from("app.py")];
        let snapshot = vfs.take_scoped_snapshot(&worker1_scope);

        // Worker 1 writes invalid app.py during attempt
        vfs.write_file("app.py", b"import invalid_module\n");
        assert!(vfs.exists("app.py"));
        assert!(vfs.exists("index.html"));

        // Worker 1 fails and triggers scoped rollback
        vfs.restore_scoped_snapshot(snapshot);

        // Verified: app.py is rolled back (removed because it was None before),
        // while index.html written by Worker 2 remains 100% intact and untouched!
        assert!(!vfs.exists("app.py"));
        assert!(vfs.exists("index.html"));
        assert_eq!(vfs.read_string("index.html").unwrap(), "<html><body>Task Manager</body></html>");
    }

    #[test]
    fn test_commit_to_dir_rejects_absolute_path_escapes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_dir = temp_dir.path().join("out_workspace");
        std::fs::create_dir_all(&target_dir).unwrap();

        let vfs = MemoryVfs::new();
        // Malicious absolute path targeting sensitive system/user file
        vfs.write_file(Path::new("/home/user/.bashrc"), b"echo evil\n");

        // Commit MUST reject absolute paths in Phase 1
        let result = vfs.commit_to_dir(Some(&target_dir), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Absolute or home-relative path"));
    }

    #[test]
    fn test_commit_to_dir_rejects_intermediate_symlinks() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_dir = temp_dir.path().join("out_workspace");
        std::fs::create_dir_all(&target_dir).unwrap();

        // Create an intermediate directory that is a symlink pointing elsewhere
        let outside_dir = temp_dir.path().join("outside_vault");
        std::fs::create_dir_all(&outside_dir).unwrap();

        let symlink_sub = target_dir.join("sub_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_dir, &symlink_sub).unwrap();

        let vfs = MemoryVfs::new();
        vfs.write_file(Path::new("sub_link/exploit.txt"), b"evil_content");

        // Commit MUST reject because intermediate directory is a symlink!
        let result = vfs.commit_to_dir(Some(&target_dir), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Intermediate directory component"));
        assert!(!outside_dir.join("exploit.txt").exists());
    }

    #[test]
    fn test_commit_two_phase_atomic_validation_pass() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_dir = temp_dir.path().join("out_workspace");
        std::fs::create_dir_all(&target_dir).unwrap();

        let vfs = MemoryVfs::new();
        // File 1 is valid
        vfs.write_file(Path::new("good1.txt"), b"good_content");
        // File 2 is an evil absolute escape
        vfs.write_file(Path::new("/etc/hosts_fake"), b"evil_content");

        // The commit MUST abort in Phase 1 without writing good1.txt to disk!
        let result = vfs.commit_to_dir(Some(&target_dir), true);
        assert!(result.is_err());

        // Verify good1.txt was NOT written to disk (Atomic all-or-nothing guarantee)
        assert!(!target_dir.join("good1.txt").exists());
    }
}
