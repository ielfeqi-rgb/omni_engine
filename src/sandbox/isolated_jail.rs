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

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use colored::Colorize;

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
        let exists = Command::new("bwrap")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if !exists {
            return false;
        }

        // Functional check: verify unprivileged user/net namespaces, /proc, /dev, and tmpfs are allowed
        let true_bin = if Path::new("/usr/bin/true").exists() {
            "/usr/bin/true"
        } else if Path::new("/bin/true").exists() {
            "/bin/true"
        } else {
            "true"
        };

        Command::new("bwrap")
            .args([
                "--ro-bind", "/", "/",
                "--proc", "/proc",
                "--dev", "/dev",
                "--size", "67108864", "--tmpfs", "/tmp",
                "--unshare-all",
                "--die-with-parent",
                "--new-session",
                true_bin,
            ])
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
        Self::run_in_jail_with_input(cmd, vfs_files, timeout, None)
    }

    /// Securely join base directory with a relative VFS path, preventing directory traversal escapes via `..`
    pub fn safe_join(base: &Path, rel: &Path) -> Result<PathBuf, String> {
        let mut clean = PathBuf::new();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(c) => clean.push(c),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    if !clean.pop() {
                        return Err(format!("Path traversal rejected (escapes base): {:?}", rel));
                    }
                }
                _ => {}
            }
        }
        let target = base.join(&clean);
        if !target.starts_with(base) {
            return Err(format!("Path traversal rejected: {:?}", rel));
        }
        Ok(target)
    }

    /// Run a command inside an isolated sandbox with the provided virtual files materialized
    /// and optional interactive/mock standard input stream.
    pub fn run_in_jail_with_input(
        cmd: &str,
        vfs_files: &[(PathBuf, Vec<u8>)],
        timeout: Duration,
        stdin_input: Option<&[u8]>,
    ) -> Result<JailExecutionResult, String> {
        let temp_dir = tempfile::tempdir()
            .map_err(|e| format!("Failed to create isolated tempdir: {}", e))?;
        let temp_path = temp_dir.path();

        // Materialize VFS files into the isolated scratch directory
        for (rel_path, content) in vfs_files {
            let target_path = Self::safe_join(temp_path, rel_path)?;
            if let Some(parent) = target_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&target_path, content)
                .map_err(|e| format!("Failed to write staged file '{}': {}", target_path.display(), e))?;
        }

        let bwrap_present = Self::is_bwrap_available();
        let allow_unsandboxed = std::env::var("OMNI_ALLOW_UNSANDBOXED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        println!("{}", "  ┌── [ISOLATED SANDBOX EXECUTION] ────────────────────────────────────".bright_cyan());
        println!("  │ Command   : {}", cmd.bright_yellow().bold());
        println!("  │ Sandbox   : {}", if bwrap_present { "Bubblewrap bwrap (Kernel unprivileged namespaces, tmpfs, new-session)" } else { "Host tempdir fallback [UNISOLATED]" });
        if !bwrap_present {
            if !allow_unsandboxed {
                eprintln!("{}", "  │ [!] SECURITY ENFORCEMENT [Fail-Closed]: Linux Bubblewrap ('bwrap') is missing or namespace creation is blocked.".bright_red().bold());
                eprintln!("{}", "  │     Untrusted LLM code execution aborted to protect the host system.".bright_yellow());
                eprintln!("{}", "  │     To explicitly permit unisolated host execution at your own risk, pass --allow-unsandboxed or set OMNI_ALLOW_UNSANDBOXED=1.".dimmed());
                return Err("SECURITY VIOLATION [Fail-Closed]: Bubblewrap sandbox unavailable. Untrusted code execution refused. Pass --allow-unsandboxed to override.".to_string());
            }
            eprintln!("{}", "  │ [!] SECURITY WARNING: Linux Bubblewrap ('bwrap') was not found in PATH.".bright_red().bold());
            eprintln!("{}", "  │     Execution is running unisolated in host tempdir because --allow-unsandboxed is active:".bright_yellow());
            eprintln!("{}", "  │     -> sudo apt install bubblewrap (or dnf install bubblewrap)".dimmed());
        }
        println!("  │ Staged    : {} file(s) materialized in RAM VFS", vfs_files.len());
        for (rel, content) in vfs_files {
            println!("  │   * {} ({} bytes)", rel.display().to_string().bright_white(), content.len());
        }

        let start_time = Instant::now();
        let (exit_code, stdout, stderr, actually_bwrap) = if bwrap_present {
            match Self::run_with_bwrap(cmd, temp_path, timeout, stdin_input) {
                Ok((code, out, err)) => {
                    if err.contains("Operation not permitted") || err.contains("Failed RTM_NEWADDR") {
                        if !allow_unsandboxed {
                            return Err(format!(
                                "SECURITY VIOLATION [Fail-Closed]: Bubblewrap sandbox execution failed due to kernel policy restrictions ({}). Untrusted execution blocked. Pass --allow-unsandboxed to permit unisolated host fallback.",
                                err.trim()
                            ));
                        }
                        let (f_code, f_out, f_err) = Self::run_fallback(cmd, temp_path, timeout, stdin_input)?;
                        (f_code, f_out, f_err, false)
                    } else {
                        (code, out, err, true)
                    }
                }
                Err(e) => {
                    if !allow_unsandboxed {
                        return Err(format!(
                            "SECURITY VIOLATION [Fail-Closed]: Bubblewrap execution encountered an unrecoverable error: {}. Untrusted execution blocked. Pass --allow-unsandboxed to permit unisolated host fallback.",
                            e
                        ));
                    }
                    let (f_code, f_out, f_err) = Self::run_fallback(cmd, temp_path, timeout, stdin_input)?;
                    (f_code, f_out, f_err, false)
                }
            }
        } else {
            let (f_code, f_out, f_err) = Self::run_fallback(cmd, temp_path, timeout, stdin_input)?;
            (f_code, f_out, f_err, false)
        };
        let duration = start_time.elapsed();

        let combined_output = format!("{}\n{}", stdout, stderr);
        let missing_dep = Self::extract_missing_dependency(&combined_output);
        let success = exit_code == Some(0) && missing_dep.is_none();

        println!("  │ Duration  : {:?}", duration);
        println!("  │ Exit Code : {}", match exit_code {
            Some(0) => "0 (SUCCESS)".bright_green().to_string(),
            Some(c) => format!("{} (FAILED)", c).bright_red().to_string(),
            None => "KILLED / TIMEOUT".bright_yellow().to_string(),
        });
        if !stdout.trim().is_empty() {
            println!("  │ STDOUT    :");
            for line in stdout.lines() {
                println!("  │   {}", line);
            }
        }
        if !stderr.trim().is_empty() {
            println!("  │ STDERR    :");
            for line in stderr.lines() {
                println!("  │   {}", line.bright_red());
            }
        }
        if let Some(ref dep) = missing_dep {
            println!("  │ {} Missing dependency: '{}'", "[DEPENDENCY COLLISION]".bright_yellow().bold(), dep.bright_red().bold());
        }
        println!("{}", "  └────────────────────────────────────────────────────────────────────".bright_cyan());

        Ok(JailExecutionResult {
            success,
            exit_code,
            stdout,
            stderr,
            missing_dependency: missing_dep,
            sandboxed_by_bwrap: actually_bwrap,
        })
    }

    /// Execute a non-interactive smoke test on a staged script (Language-agnostic)
    pub fn smoke_test_script(
        target_file: &str,
        vfs_files: &[(PathBuf, Vec<u8>)],
        timeout: Duration,
    ) -> Result<JailExecutionResult, String> {
        let p = Path::new(target_file);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let cmd = match ext {
            "py" => format!("python3 {}", target_file),
            "js" => format!("node {}", target_file),
            "sh" | "bash" => format!("bash {}", target_file),
            "rb" => format!("ruby {}", target_file),
            _ => crate::planner::swarm_coordinator::SwarmCoordinator::resolve_verification_command(target_file),
        };
        Self::run_in_jail(&cmd, vfs_files, timeout)
    }

    /// Run with Bubblewrap (`bwrap`) isolation
    fn run_with_bwrap(
        cmd: &str,
        workspace: &Path,
        timeout: Duration,
        stdin_input: Option<&[u8]>,
    ) -> Result<(Option<i32>, String, String), String> {
        let mut bwrap = Command::new("bwrap");

        // Read-only system mounts
        bwrap.args(["--ro-bind", "/usr", "/usr"]);
        if Path::new("/etc").exists() {
            bwrap.args(["--ro-bind-try", "/etc", "/etc"]);
        }

        // Symlinks or binds for root bin/lib directories
        if Path::new("/bin").is_symlink() {
            bwrap.args(["--symlink", "usr/bin", "/bin"]);
        } else if Path::new("/bin").exists() {
            bwrap.args(["--ro-bind-try", "/bin", "/bin"]);
        }

        if Path::new("/lib").is_symlink() {
            bwrap.args(["--symlink", "usr/lib", "/lib"]);
        } else if Path::new("/lib").exists() {
            bwrap.args(["--ro-bind-try", "/lib", "/lib"]);
        }

        if Path::new("/lib64").is_symlink() {
            bwrap.args(["--symlink", "usr/lib64", "/lib64"]);
        } else if Path::new("/lib64").exists() {
            bwrap.args(["--ro-bind-try", "/lib64", "/lib64"]);
        }

        // Read-only user local packages (~/.local/lib and ~/.local/lib64) so user-installed packages are accessible in sandbox
        if let Ok(home) = std::env::var("HOME") {
            bwrap.args(["--setenv", "HOME", &home]);
            let user_local_lib = Path::new(&home).join(".local").join("lib");
            if user_local_lib.exists() {
                if let Some(user_local_str) = user_local_lib.to_str() {
                    bwrap.args(["--ro-bind-try", user_local_str, user_local_str]);
                }
            }
            let user_local_lib64 = Path::new(&home).join(".local").join("lib64");
            if user_local_lib64.exists() {
                if let Some(user_local_str) = user_local_lib64.to_str() {
                    bwrap.args(["--ro-bind-try", user_local_str, user_local_str]);
                }
            }
        }

        // Ephemeral device & proc mounts with memory caps (64MB cap on tmpfs)
        bwrap.args(["--proc", "/proc"]);
        bwrap.args(["--dev", "/dev"]);
        bwrap.args(["--size", "67108864", "--tmpfs", "/tmp"]);

        // Isolated workspace directory
        let ws_str = workspace.to_str().ok_or_else(|| "Invalid UTF-8 in workspace path".to_string())?;
        bwrap.args(["--bind", ws_str, "/workspace"]);
        bwrap.args(["--chdir", "/workspace"]);

        // Security flags: unshare network, pid, ipc, user; die when parent terminates;
        // --new-session creates a new process group/session detached from TTY, preventing TIOCSTI keystroke injection.
        bwrap.args(["--unshare-all", "--die-with-parent", "--new-session"]);

        // Pass headless display env var, PATH, and unbuffered I/O for Python & subprocesses
        bwrap.args(["--setenv", "DISPLAY", ":99"]);
        bwrap.args(["--setenv", "PYTHONUNBUFFERED", "1"]);
        bwrap.args(["--setenv", "PATH", "/usr/local/bin:/usr/bin:/bin"]);

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            bwrap.process_group(0);
            unsafe {
                bwrap.pre_exec(|| {
                    // Max single file output size: 128MB (stops disk fill loops)
                    let fsize_limit = libc::rlimit {
                        rlim_cur: 128 * 1024 * 1024,
                        rlim_max: 128 * 1024 * 1024,
                    };
                    let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &fsize_limit);
                    Ok(())
                });
            }
        }

        // Run command via sh
        bwrap.args(["sh", "-c", cmd]);

        bwrap.stdout(Stdio::piped());
        bwrap.stderr(Stdio::piped());

        if stdin_input.is_some() {
            bwrap.stdin(Stdio::piped());
        } else {
            bwrap.stdin(Stdio::null());
        }

        let mut child = bwrap
            .spawn()
            .map_err(|e| format!("Failed to spawn bwrap: {}", e))?;

        if let Some(data) = stdin_input {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(data);
                let _ = stdin.flush();
            }
        }

        let (exit_code, stdout, stderr) = Self::wait_with_timeout(&mut child, cmd, timeout)?;
        Ok((exit_code, stdout, stderr))
    }

    /// Fallback execution in an isolated temporary directory without bwrap
    fn run_fallback(
        cmd: &str,
        workspace: &Path,
        timeout: Duration,
        stdin_input: Option<&[u8]>,
    ) -> Result<(Option<i32>, String, String), String> {
        let mut process = Command::new("sh");
        process.arg("-c").arg(cmd);
        process.current_dir(workspace);
        process.env("DISPLAY", ":99");
        process.env("PYTHONUNBUFFERED", "1");
        process.stdout(Stdio::piped());
        process.stderr(Stdio::piped());

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.process_group(0);
            unsafe {
                process.pre_exec(|| {
                    // Max single file size: 128MB
                    let fsize_limit = libc::rlimit {
                        rlim_cur: 128 * 1024 * 1024,
                        rlim_max: 128 * 1024 * 1024,
                    };
                    let _ = libc::setrlimit(libc::RLIMIT_FSIZE, &fsize_limit);
                    Ok(())
                });
            }
        }

        if stdin_input.is_some() {
            process.stdin(Stdio::piped());
        } else {
            process.stdin(Stdio::null());
        }

        let mut child = process
            .spawn()
            .map_err(|e| format!("Failed to spawn shell fallback: {}", e))?;

        if let Some(data) = stdin_input {
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(data);
                let _ = stdin.flush();
            }
        }

        let (exit_code, stdout, stderr) = Self::wait_with_timeout(&mut child, cmd, timeout)?;
        Ok((exit_code, stdout, stderr))
    }

    /// Wait for child process with timeout to prevent hung scripts
    fn wait_with_timeout(
        child: &mut std::process::Child,
        cmd: &str,
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
                        #[cfg(unix)]
                        {
                            let pid = child.id() as libc::pid_t;
                            unsafe {
                                let _ = libc::killpg(pid, libc::SIGKILL);
                            }
                        }
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

                        // If the command explicitly requested a server daemon and started listening successfully:
                        let is_server_cmd = cmd.contains("flask") || cmd.contains("http.server") || cmd.contains("uvicorn")
                            || cmd.contains("gunicorn") || cmd.contains("node ") || cmd.contains("serve");
                        let is_server_listening = is_server_cmd && (
                            out_str.contains("Running on http") || out_str.contains("Serving HTTP on")
                            || err_str.contains("Running on http") || err_str.contains("Serving HTTP on")
                            || out_str.contains("* Serving Flask app") || err_str.contains("* Serving Flask app")
                            || out_str.contains("Uvicorn running on") || err_str.contains("Uvicorn running on")
                        );

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
            Duration::from_secs(15),
        ).expect("Smoke test should run");

        assert!(!res.success);
        assert_eq!(
            res.missing_dependency,
            Some("non_existent_dep_xyz123".to_string())
        );
    }

    #[test]
    fn test_safe_join_path_traversal_rejection() {
        let base = Path::new("/tmp/omni_sandbox_test");
        let evil_path = Path::new("../../etc/passwd");
        assert!(IsolatedJail::safe_join(base, evil_path).is_err());

        let nested_evil = Path::new("sub/../../../../etc/shadow");
        assert!(IsolatedJail::safe_join(base, nested_evil).is_err());

        let valid_path = Path::new("src/main.rs");
        let target = IsolatedJail::safe_join(base, valid_path).expect("Should be valid");
        assert_eq!(target, base.join("src/main.rs"));
    }
}
