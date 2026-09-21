use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

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

    /// Preload or mount a real file from host into RAM sandbox (read-only copy)
    pub fn preload_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = path.as_ref().to_path_buf();
        let mut original = self.original_snapshots.write().unwrap();
        let mut active = self.files.write().unwrap();
        original.insert(path_buf.clone(), content.to_vec());
        active.insert(path_buf, content.to_vec());
    }

    /// Model writes or edits a file inside the isolated RAM VFS
    pub fn write_file<P: AsRef<Path>>(&self, path: P, content: &[u8]) {
        let path_buf = path.as_ref().to_path_buf();
        let mut active = self.files.write().unwrap();
        active.insert(path_buf, content.to_vec());
    }

    /// Read file content from RAM VFS
    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> Option<Vec<u8>> {
        let path_buf = path.as_ref().to_path_buf();
        let active = self.files.read().unwrap();
        active.get(&path_buf).cloned()
    }

    /// Read file as UTF-8 string
    pub fn read_string<P: AsRef<Path>>(&self, path: P) -> Option<String> {
        self.read_file(path).and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Check if file exists in RAM VFS
    pub fn exists<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = path.as_ref().to_path_buf();
        let active = self.files.read().unwrap();
        active.contains_key(&path_buf)
    }

    /// Delete file inside RAM VFS
    pub fn delete_file<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_buf = path.as_ref().to_path_buf();
        let mut active = self.files.write().unwrap();
        active.remove(&path_buf).is_some()
    }

    /// Compute staged changes (Diffs) between original snapshots and active sandbox modifications.
    pub fn generate_staged_diffs(&self) -> Vec<StagedDiff> {
        let active = self.files.read().unwrap();
        let original = self.original_snapshots.read().unwrap();
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
            match diff.change_type {
                FileChangeType::Created | FileChangeType::Modified => {
                    if let Some(parent) = diff.path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Some(content) = self.read_file(&diff.path) {
                        std::fs::write(&diff.path, content)
                            .map_err(|e| format!("Failed to write to host file {:?}: {}", diff.path, e))?;
                        committed += 1;
                    }
                }
                FileChangeType::Deleted => {
                    if diff.path.exists() {
                        std::fs::remove_file(&diff.path)
                            .map_err(|e| format!("Failed to remove host file {:?}: {}", diff.path, e))?;
                        committed += 1;
                    }
                }
            }
        }

        // Update original snapshots to reflect committed state
        let mut original = self.original_snapshots.write().unwrap();
        let active = self.files.read().unwrap();
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
}
