use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
    Blocked { reason: String },
}

#[derive(Debug, Clone)]
pub struct TerminalJob {
    pub job_id: u64,
    pub command: String,
    pub status: JobStatus,
    pub started_at: Instant,
    pub finished_at: Option<Instant>,
}

/// Persistent Terminal Session Bridge.
/// Provides an interactive PTY/IPC bridge for the AI agent to:
/// - Dispatch commands to an active host shell environment.
/// - Stream and buffer live logs into a volatile in-memory ring buffer.
/// - Poll execution status and inspect exit codes without hanging the runtime.
/// - Enforce safety guardrails against destructive commands.
#[derive(Clone)]
pub struct TerminalSessionBridge {
    max_buffer_lines: usize,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    jobs: Arc<Mutex<Vec<TerminalJob>>>,
    next_job_id: Arc<AtomicU64>,
}

impl TerminalSessionBridge {
    pub fn new(max_buffer_lines: usize) -> Self {
        Self {
            max_buffer_lines: if max_buffer_lines == 0 { 1000 } else { max_buffer_lines },
            log_buffer: Arc::new(Mutex::new(VecDeque::new())),
            jobs: Arc::new(Mutex::new(Vec::new())),
            next_job_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Check if a command violates critical system safety policies
    pub fn validate_safety(&self, command: &str) -> Result<(), String> {
        let trimmed = command.trim();
        let lower = trimmed.to_lowercase();

        let forbidden_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "mkfs",
            "dd if=/dev/zero",
            "dd if=/dev/random",
            ":(){ :|:& };:",
            "> /dev/sda",
            "> /dev/nvme",
            "chmod -r 777 /",
            "shutdown",
            "reboot",
            "init 0",
        ];

        for pattern in forbidden_patterns {
            if lower.contains(pattern) {
                return Err(format!("Command rejected by safety gate: contains dangerous pattern '{}'", pattern));
            }
        }

        Ok(())
    }

    /// Execute a command in the background, stream its logs into the ring buffer,
    /// and track its lifecycle.
    pub fn execute(&self, command: &str) -> (u64, JobStatus) {
        let job_id = self.next_job_id.fetch_add(1, Ordering::SeqCst);

        // 1. Safety validation
        if let Err(reason) = self.validate_safety(command) {
            let job = TerminalJob {
                job_id,
                command: command.to_string(),
                status: JobStatus::Blocked { reason: reason.clone() },
                started_at: Instant::now(),
                finished_at: Some(Instant::now()),
            };
            self.jobs.lock().unwrap().push(job);
            self.append_log(format!("[SECURITY] Job #{}: {}", job_id, reason));
            return (job_id, JobStatus::Blocked { reason });
        }

        self.append_log(format!("[TERMINAL] >>> Job #{}: Executing '{}'", job_id, command));

        let initial_job = TerminalJob {
            job_id,
            command: command.to_string(),
            status: JobStatus::Running,
            started_at: Instant::now(),
            finished_at: None,
        };
        self.jobs.lock().unwrap().push(initial_job);

        // 2. Spawn execution worker thread
        let log_buffer_clone = self.log_buffer.clone();
        let max_lines = self.max_buffer_lines;
        let jobs_clone = self.jobs.clone();
        let cmd_string = command.to_string();

        std::thread::spawn(move || {
            let mut child = match Command::new("sh")
                .arg("-c")
                .arg(&cmd_string)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(err) => {
                    let mut jobs = jobs_clone.lock().unwrap();
                    if let Some(j) = jobs.iter_mut().find(|j| j.job_id == job_id) {
                        j.status = JobStatus::Failed { error: err.to_string() };
                        j.finished_at = Some(Instant::now());
                    }
                    let mut buf = log_buffer_clone.lock().unwrap();
                    buf.push_back(format!("[ERROR] Job #{}: Failed to spawn process: {}", job_id, err));
                    if buf.len() > max_lines {
                        buf.pop_front();
                    }
                    return;
                }
            };

            // Stream stdout in real-time
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().flatten() {
                    let mut buf = log_buffer_clone.lock().unwrap();
                    buf.push_back(format!("[Job #{}] {}", job_id, line));
                    if buf.len() > max_lines {
                        buf.pop_front();
                    }
                }
            }

            // Stream stderr in real-time
            if let Some(stderr) = child.stderr.take() {
                let reader = BufReader::new(stderr);
                for line in reader.lines().flatten() {
                    let mut buf = log_buffer_clone.lock().unwrap();
                    buf.push_back(format!("[Job #{} STDERR] {}", job_id, line));
                    if buf.len() > max_lines {
                        buf.pop_front();
                    }
                }
            }

            let status_res = child.wait();
            let mut jobs = jobs_clone.lock().unwrap();
            if let Some(j) = jobs.iter_mut().find(|j| j.job_id == job_id) {
                j.finished_at = Some(Instant::now());
                match status_res {
                    Ok(exit_status) => {
                        let code = exit_status.code().unwrap_or(-1);
                        j.status = JobStatus::Completed { exit_code: code };
                        let mut buf = log_buffer_clone.lock().unwrap();
                        buf.push_back(format!("[TERMINAL] Job #{}: Process exited with status code {}", job_id, code));
                        if buf.len() > max_lines {
                            buf.pop_front();
                        }
                    }
                    Err(e) => {
                        j.status = JobStatus::Failed { error: e.to_string() };
                        let mut buf = log_buffer_clone.lock().unwrap();
                        buf.push_back(format!("[TERMINAL] Job #{}: Process wait error: {}", job_id, e));
                        if buf.len() > max_lines {
                            buf.pop_front();
                        }
                    }
                }
            }
        });

        (job_id, JobStatus::Running)
    }

    /// Read the latest N lines from the circular log buffer
    pub fn get_logs(&self, tail_lines: usize) -> Vec<String> {
        let buf = self.log_buffer.lock().unwrap();
        let count = if tail_lines == 0 { buf.len() } else { tail_lines.min(buf.len()) };
        buf.iter().rev().take(count).rev().cloned().collect()
    }

    /// Clear in-memory log buffer
    pub fn clear_logs(&self) {
        let mut buf = self.log_buffer.lock().unwrap();
        buf.clear();
    }

    /// Poll status of a specific job
    pub fn poll_status(&self, job_id: u64) -> Option<JobStatus> {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().find(|j| j.job_id == job_id).map(|j| j.status.clone())
    }

    /// Get total number of running jobs
    pub fn active_jobs_count(&self) -> usize {
        let jobs = self.jobs.lock().unwrap();
        jobs.iter().filter(|j| j.status == JobStatus::Running).count()
    }

    fn append_log(&self, line: String) {
        let mut buf = self.log_buffer.lock().unwrap();
        buf.push_back(line);
        if buf.len() > self.max_buffer_lines {
            buf.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_terminal_bridge_echo_execution() {
        let bridge = TerminalSessionBridge::new(100);
        let (job_id, initial_status) = bridge.execute("echo 'HELLO_OMNI_TERMINAL'");
        assert_eq!(job_id, 1);
        assert_eq!(initial_status, JobStatus::Running);

        // Wait up to 1 second for thread to execute and flush logs
        for _ in 0..20 {
            if let Some(JobStatus::Completed { exit_code }) = bridge.poll_status(1) {
                assert_eq!(exit_code, 0);
                break;
            }
            sleep(Duration::from_millis(50));
        }

        let logs = bridge.get_logs(10);
        assert!(logs.iter().any(|l| l.contains("HELLO_OMNI_TERMINAL")), "Logs must capture echo output");
    }

    #[test]
    fn test_terminal_bridge_blocks_dangerous_command() {
        let bridge = TerminalSessionBridge::new(100);
        let (_job_id, status) = bridge.execute("rm -rf / --no-preserve-root");
        match status {
            JobStatus::Blocked { reason } => {
                assert!(reason.contains("dangerous pattern"));
            }
            _ => panic!("Dangerous command must be blocked!"),
        }
    }

    #[test]
    fn test_terminal_bridge_captures_stderr() {
        let bridge = TerminalSessionBridge::new(100);
        let (job_id, _) = bridge.execute("sh -c 'echo \"AN_ERROR_MESSAGE\" >&2; exit 42'");

        for _ in 0..20 {
            if let Some(JobStatus::Completed { exit_code }) = bridge.poll_status(job_id) {
                assert_eq!(exit_code, 42);
                break;
            }
            sleep(Duration::from_millis(50));
        }

        let logs = bridge.get_logs(10);
        assert!(logs.iter().any(|l| l.contains("AN_ERROR_MESSAGE")), "Logs must capture stderr output");
    }
}
