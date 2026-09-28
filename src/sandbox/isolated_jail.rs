use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Result of executing a command in the isolated jail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JailExecutionResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub missing_dependency: Option<String>,
    pub sandboxed_by_bwrap: bool,
}

/// Isolated host-grounded execution sandbox.
/// Uses Bubblewrap (`bwrap`) when available for unprivileged container isolation
/// with read-only root filesystems, private `/tmp`, and isolated network/PID/IPC namespaces.
/// Automatically falls back to an isolated temporary directory if `bwrap` is not present.
pub struct IsolatedJail;

impl IsolatedJail {
    /// Check if `bwrap` is installed and functioning on the host.
    pub fn is_bwrap_available() -> bool {
        Command::new("bwrap")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Run a command inside an isolated sandbox with the provided virtual files materialized.
    ///
    /// The files in `vfs_files` are written into an ephemeral scratch workspace.
    /// If `bwrap` is available, the command is executed inside an unprivileged mount namespace
    /// where `/usr`, `/etc`, and system libraries are strictly read-only, `/workspace` is writable
    /// only within the ephemeral directory, and all other namespaces (net, pid, ipc, user) are unshared.
    pub fn run_in_jail(
        cmd: &str,
        vfs_files: &[(PathBuf, Vec<u8>)],
        timeout: Duration,
    ) -> Result<JailExecutionResult, String> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create isolated tempdir: {}", e))?;
        let temp_path = temp_dir.path();

        // Materialize VFS files into the isolated scratch directory
        for (rel_path, content) in vfs_files {
            // Strip any leading slashes or normalize
            let clean_rel = if rel_path.is_absolute() {
                rel_path.strip_prefix("/").unwrap_or(rel_path)
            } else {
                rel_path.as_path()
            };
            let target_path = temp_path.join(clean_rel);
            if let Some(parent) = target_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&target_path, content)
                .map_err(|e| format!("Failed to write staged file '{}': {}", target_path.display(), e))?;
        }

        let bwrap_present = Self::is_bwrap_available();

        let _start_time = Instant::now();
        let (exit_code, stdout, stderr) = if bwrap_present {
            Self::run_with_bwrap(cmd, temp_path, timeout)?
        } else {
            Self::run_fallback(cmd, temp_path, timeout)?
        };

        let combined_output = format!("{}\n{}", stdout, stderr);
        let missing_dep = Self::extract_missing_dependency(&combined_output);

        let success = exit_code == Some(0) && missing_dep.is_none();

        Ok(JailExecutionResult {
            success,
            exit_code,
            stdout,
            stderr,
            missing_dependency: missing_dep,
            sandboxed_by_bwrap: bwrap_present,
        })
    }

    /// Execute a non-interactive smoke test on a staged script (Python, Shell, etc.)
    pub fn smoke_test_script(
        target_file: &str,
        vfs_files: &[(PathBuf, Vec<u8>)],
        timeout: Duration,
    ) -> Result<JailExecutionResult, String> {
        let cmd = if target_file.ends_with(".py") {
            let stem = target_file.trim_end_matches(".py");
            format!(
                "python3 -c \"import sys, os; sys.path.insert(0, '.'); os.environ['DISPLAY'] = ':99'; import {}\"",
                stem
            )
        } else if target_file.ends_with(".sh") {
            format!("bash -n {}", target_file)
        } else if target_file.ends_with(".js") {
            format!("node --check {}", target_file)
        } else {
            // Default generic syntax check or file presence check
            format!("test -s {}", target_file)
        };

        Self::run_in_jail(&cmd, vfs_files, timeout)
    }

    /// Run with Bubblewrap (`bwrap`) isolation
    fn run_with_bwrap(
        cmd: &str,
        workspace: &Path,
        timeout: Duration,
    ) -> Result<(Option<i32>, String, String), String> {
        let mut bwrap = Command::new("bwrap");

        // Read-only system mounts
        bwrap.args(["--ro-bind", "/usr", "/usr"]);
        if Path::new("/etc").exists() {
            bwrap.args(["--ro-bind", "/etc", "/etc"]);
        }

        // Symlinks or binds for root bin/lib directories
        if Path::new("/bin").is_symlink() {
            bwrap.args(["--symlink", "usr/bin", "/bin"]);
        } else if Path::new("/bin").exists() {
            bwrap.args(["--ro-bind", "/bin", "/bin"]);
        }

        if Path::new("/lib").is_symlink() {
            bwrap.args(["--symlink", "usr/lib", "/lib"]);
        } else if Path::new("/lib").exists() {
            bwrap.args(["--ro-bind", "/lib", "/lib"]);
        }

        if Path::new("/lib64").is_symlink() {
            bwrap.args(["--symlink", "usr/lib64", "/lib64"]);
        } else if Path::new("/lib64").exists() {
            bwrap.args(["--ro-bind", "/lib64", "/lib64"]);
        }

        // Read-only user local packages (~/.local) so pip installs are accessible in sandbox
        if let Ok(home) = std::env::var("HOME") {
            let user_local = Path::new(&home).join(".local");
            if user_local.exists() {
                if let Some(user_local_str) = user_local.to_str() {
                    bwrap.args(["--ro-bind", user_local_str, user_local_str]);
                }
            }
        }

        // Ephemeral device & proc mounts
        bwrap.args(["--proc", "/proc"]);
        bwrap.args(["--dev", "/dev"]);
        bwrap.args(["--tmpfs", "/tmp"]);

        // Isolated workspace directory
        let ws_str = workspace.to_str().ok_or_else(|| "Invalid UTF-8 in workspace path".to_string())?;
        bwrap.args(["--bind", ws_str, "/workspace"]);
        bwrap.args(["--chdir", "/workspace"]);

        // Security flags: unshare network, pid, ipc, user; die when parent terminates
        bwrap.args(["--unshare-all", "--die-with-parent"]);

        // Pass headless display env var for GUI scripts
        bwrap.args(["--setenv", "DISPLAY", ":99"]);
        bwrap.args(["--setenv", "PYTHONUNBUFFERED", "1"]);

        // Run command via sh
        bwrap.args(["sh", "-c", cmd]);

        bwrap.stdout(Stdio::piped());
        bwrap.stderr(Stdio::piped());

        let mut child = bwrap
            .spawn()
            .map_err(|e| format!("Failed to spawn bwrap: {}", e))?;

        let (exit_code, stdout, stderr) = Self::wait_with_timeout(&mut child, timeout)?;
        Ok((exit_code, stdout, stderr))
    }

    /// Fallback execution in an isolated temporary directory without bwrap
    fn run_fallback(
        cmd: &str,
        workspace: &Path,
        timeout: Duration,
    ) -> Result<(Option<i32>, String, String), String> {
        let mut process = Command::new("sh");
        process.arg("-c").arg(cmd);
        process.current_dir(workspace);
        process.env("DISPLAY", ":99");
        process.env("PYTHONUNBUFFERED", "1");
        process.stdout(Stdio::piped());
        process.stderr(Stdio::piped());

        let mut child = process
            .spawn()
            .map_err(|e| format!("Failed to spawn shell fallback: {}", e))?;

        let (exit_code, stdout, stderr) = Self::wait_with_timeout(&mut child, timeout)?;
        Ok((exit_code, stdout, stderr))
    }

    /// Wait for child process with timeout to prevent hung scripts
    fn wait_with_timeout(
        child: &mut std::process::Child,
        timeout: Duration,
    ) -> Result<(Option<i32>, String, String), String> {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(50);

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut stdout_buf = Vec::new();
                    let mut stderr_buf = Vec::new();
                    if let Some(mut out) = child.stdout.take() {
                        let _ = std::io::Read::read_to_end(&mut out, &mut stdout_buf);
                    }
                    if let Some(mut err) = child.stderr.take() {
                        let _ = std::io::Read::read_to_end(&mut err, &mut stderr_buf);
                    }
                    return Ok((
                        status.code(),
                        String::from_utf8_lossy(&stdout_buf).to_string(),
                        String::from_utf8_lossy(&stderr_buf).to_string(),
                    ));
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        let mut stdout_buf = Vec::new();
                        let mut stderr_buf = Vec::new();
                        if let Some(mut out) = child.stdout.take() {
                            let _ = std::io::Read::read_to_end(&mut out, &mut stdout_buf);
                        }
                        if let Some(mut err) = child.stderr.take() {
                            let _ = std::io::Read::read_to_end(&mut err, &mut stderr_buf);
                        }
                        let out_str = String::from_utf8_lossy(&stdout_buf).to_string();
                        let err_str = String::from_utf8_lossy(&stderr_buf).to_string();

                        // If the process was a server/daemon that started and listened successfully, treat as success!
                        let is_server_listening = out_str.contains("Running on") || out_str.contains("Serving HTTP")
                            || err_str.contains("Running on") || err_str.contains("Serving HTTP")
                            || out_str.contains("Press CTRL+C") || err_str.contains("Press CTRL+C")
                            || out_str.contains("* Serving Flask") || err_str.contains("* Serving Flask");

                        if is_server_listening {
                            return Ok((Some(0), out_str, err_str));
                        }

                        return Ok((
                            Some(124), // standard timeout exit code
                            out_str,
                            format!("Execution timed out after {:?}: {}", timeout, err_str),
                        ));
                    }
                    std::thread::sleep(poll_interval);
                }
                Err(e) => return Err(format!("Error polling child process: {}", e)),
            }
        }
    }

    /// Extract missing dependency name from command execution output or error trace
    pub fn extract_missing_dependency(text: &str) -> Option<String> {
        // 1. Python ModuleNotFoundError: No module named 'tkinter'
        if let Some(idx) = text.find("No module named ") {
            let sub = &text[idx + "No module named ".len()..];
            let sub = sub.trim_start();
            let mut quote = None;
            if let Some(first_char) = sub.chars().next() {
                let dep = if first_char == '\'' || first_char == '"' {
                    quote = Some(first_char);
                    &sub[1..]
                } else {
                    sub
                };
                let end_idx = if let Some(q) = quote {
                    dep.find(q).unwrap_or_else(|| {
                        dep.split_whitespace().next().map(|s| s.len()).unwrap_or(dep.len())
                    })
                } else {
                    dep.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
                        .unwrap_or(dep.len())
                };
                let candidate = dep[..end_idx].trim().to_string();
                if !candidate.is_empty() {
                    return Some(candidate);
                }
            }
        }

        // 2. Python ImportError: cannot import name ... from 'xxx'
        if text.contains("ImportError") {
            if let Some(from_idx) = text.find("from '") {
                let sub = &text[from_idx + 6..];
                if let Some(end) = sub.find('\'') {
                    let dep = sub[..end].trim().to_string();
                    if !dep.is_empty() {
                        return Some(dep);
                    }
                }
            }
        }

        // 3. Node.js Cannot find module 'xxx'
        if let Some(idx) = text.find("Cannot find module '") {
            let sub = &text[idx + "Cannot find module '".len()..];
            if let Some(end) = sub.find('\'') {
                let dep = sub[..end].trim().to_string();
                if !dep.is_empty() {
                    return Some(dep);
                }
            }
        }

        // 4. Linux command not found
        if text.contains("command not found") || text.contains(": not found") {
            const IGNORED_COMMANDS: &[&str] = &[
                "run", "exec", "terminal", "sh", "bash", "cmd", "action", "command",
                "python", "python3", "pytest", "sudo", "exit", "test", "true", "false",
            ];
            for line in text.lines() {
                let candidate = if let Some(idx) = line.find("command not found:") {
                    let sub = line[idx + "command not found:".len()..].trim();
                    sub.split_whitespace().next()
                } else if let Some(idx) = line.find(": command not found") {
                    let prefix = line[..idx].trim();
                    prefix.split_whitespace().last()
                } else if let Some(idx) = line.find(": not found") {
                    let prefix = line[..idx].trim();
                    prefix.split_whitespace().last()
                } else {
                    None
                };

                if let Some(word) = candidate {
                    let clean = word.trim_matches(|c| c == '\'' || c == '"' || c == ':').to_string();
                    if !clean.is_empty() && !IGNORED_COMMANDS.contains(&clean.to_lowercase().as_str()) {
                        return Some(clean);
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bwrap_detection() {
        // On systems where bwrap is present, this should be true
        let available = IsolatedJail::is_bwrap_available();
        println!("bwrap available on host: {}", available);
    }

    #[test]
    fn test_isolated_execution_clean() {
        let vfs_files = vec![
            (PathBuf::from("hello.txt"), b"Hello from isolated sandbox!".to_vec()),
        ];
        let res = IsolatedJail::run_in_jail(
            "cat hello.txt",
            &vfs_files,
            Duration::from_secs(5),
        ).expect("Execution should succeed");

        assert!(res.success);
        assert_eq!(res.exit_code, Some(0));
        assert!(res.stdout.contains("Hello from isolated sandbox!"));
        assert!(res.missing_dependency.is_none());
    }

    #[test]
    fn test_isolated_filesystem_protection() {
        // Verify that writing to /usr or /etc is blocked inside bwrap
        if IsolatedJail::is_bwrap_available() {
            let res = IsolatedJail::run_in_jail(
                "touch /usr/bin/should_fail.txt",
                &[],
                Duration::from_secs(5),
            ).expect("Should execute with error");

            assert!(!res.success);
            assert!(res.stderr.contains("Read-only") || res.exit_code != Some(0));
        }
    }

    #[test]
    fn test_missing_dependency_extraction() {
        let err1 = "Traceback (most recent call last):\n  File \"GameLoop.py\", line 1, in <module>\n    import tkinter as tk\nModuleNotFoundError: No module named 'tkinter'";
        assert_eq!(
            IsolatedJail::extract_missing_dependency(err1),
            Some("tkinter".to_string())
        );

        let err2 = "sh: line 1: nonexisting_tool: command not found";
        assert_eq!(
            IsolatedJail::extract_missing_dependency(err2),
            Some("nonexisting_tool".to_string())
        );

        let err3 = "node:internal/modules/cjs/loader:1147\nError: Cannot find module 'express'";
        assert_eq!(
            IsolatedJail::extract_missing_dependency(err3),
            Some("express".to_string())
        );
    }

    #[test]
    fn test_smoke_test_script_detects_missing_module() {
        let script = b"import non_existent_dep_xyz123\nprint('running')\n".to_vec();
        let vfs_files = vec![(PathBuf::from("test_script.py"), script)];

        let res = IsolatedJail::smoke_test_script(
            "test_script.py",
            &vfs_files,
            Duration::from_secs(5),
        ).expect("Smoke test should run");

        assert!(!res.success);
        assert_eq!(
            res.missing_dependency,
            Some("non_existent_dep_xyz123".to_string())
        );
    }
}
