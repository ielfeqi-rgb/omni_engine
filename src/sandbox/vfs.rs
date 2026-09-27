use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct StagedDiff {
    pub path: PathBuf,
    pub change_type: FileChangeType,
    pub original_content: Option<String>,
    pub new_content: Option<String>,
}

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

    /// Preload or mount a real file from host into RAM sandbox (read-only copy)
    pub fn preload_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = Self::resolve_path(path);
        let mut original = self.original_snapshots.write();
        let mut active = self.files.write();
        original.insert(path_buf.clone(), content.to_vec());
        active.insert(path_buf, content.to_vec());
    }

    /// Model writes or edits a file inside the isolated RAM VFS
    pub fn write_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = Self::resolve_path(path);
        let mut active = self.files.write();
        active.insert(path_buf, content.to_vec());
    }

    /// Take a baseline snapshot of a specific file in active VFS for diffing
    pub fn snapshot_file<P: AsRef<Path>>(&self, path: P) {
        let path_buf = Self::resolve_path(path);
        let active = self.files.read();
        if let Some(content) = active.get(&path_buf) {
            let mut original = self.original_snapshots.write();
            original.insert(path_buf, content.clone());
        }
    }

    /// Read file content from RAM VFS
    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> Option<Vec<u8>> {
        let path_buf = Self::resolve_path(path);
        let active = self.files.read();
        active.get(&path_buf).cloned()
    }

    /// Read file as UTF-8 string
    pub fn read_string<P: AsRef<Path>>(&self, path: P) -> Option<String> {
        self.read_file(path).and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Check if file exists in RAM VFS
    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = Self::resolve_path(path);
        let active = self.files.read();
        active.contains_key(&path_buf)
    }

    /// Delete file inside RAM VFS
    pub fn delete_file<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = Self::resolve_path(path);
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
        let path_buf = Self::resolve_path(path);
        let active = self.files.read();
        active.get(&path_buf).map(|b| b.len()).unwrap_or(0)
    }

    /// Surgically patch a file in RAM VFS by replacing the first occurrence of `target` with `replacement`
    pub fn patch_file<P: AsRef<Path>>(&self, path: P, target: &str, replacement: &str) -> Result<bool, String> {
        let path_buf = Self::resolve_path(path);
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
        let path_buf = Self::resolve_path(path);
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

    /// Commit staged diffs to actual host disk ONLY upon explicit user confirmation.
    pub fn commit_to_host(&self, user_authorized: bool) -> Result<usize, String> {
        if !user_authorized {
            return Err("Authorization Denied: Cannot commit sandbox VFS to host disk without user approval".to_string());
        }

        let diffs = self.generate_staged_diffs();
        let mut committed = 0;

        for diff in &diffs {
            let target_path = Self::resolve_path(&diff.path);
            match diff.change_type {
                FileChangeType::Created | FileChangeType::Modified => {
                    if let Some(parent) = target_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Some(content) = self.read_file(&diff.path) {
                        std::fs::write(&target_path, content)
                            .map_err(|e| format!("Failed to write to host file {:?}: {}", target_path, e))?;
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
}
