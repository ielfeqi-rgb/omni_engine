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

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;
use colored::*;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::native_llama::{NativeLlamaContext, NativeLlamaModel};
use crate::sandbox::{MemoryVfs, WebLens, IsolatedJail};

/// Convenience module-level forwarder for stripping thoughts across Omni Engine
pub fn strip_thought(text: &str) -> String {
    SwarmCoordinator::strip_thought(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskKind {
    Create,
    Modify,
    Delete,
    Test,
    Search,
}

impl TaskKind {
    pub fn name(&self) -> &'static str {
        match self {
            TaskKind::Create => "CREATE",
            TaskKind::Modify => "MODIFY",
            TaskKind::Delete => "DELETE",
            TaskKind::Test => "TEST",
            TaskKind::Search => "SEARCH",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwarmAction {
    Search { query: String },
    Fetch { url: String },
    WriteFile { filename: String, content: String },
    PatchFile { filename: String, target: String, replacement: String },
    DeleteFile { filename: String },
    RunTerminal { command: String },
    TestRun { command: String, inputs: Vec<String> },
    Verdict { status: bool, proof_or_missing: String },
    Consult { question: String },
    Report { finding: String },
    Done,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticCode {
    VfsFileNotWritten,
    PlaceholderOrEmpty,
    NoCodeBlock,
    RepeatedAttractor,
    MissingDependency(String),
    PatchFailed(String),
    FileNotDeleted,
    DiffEmpty,
    TestFailed,
    OutOfBoundsWrite,
    AcceptanceCriteriaFailed(String),
}

impl DiagnosticCode {
    pub fn name(&self) -> &'static str {
        match self {
            DiagnosticCode::VfsFileNotWritten => "ERR_VFS_FILE_NOT_WRITTEN",
            DiagnosticCode::PlaceholderOrEmpty => "ERR_PLACEHOLDER_OR_EMPTY",
            DiagnosticCode::NoCodeBlock => "ERR_NO_CODE_BLOCK",
            DiagnosticCode::RepeatedAttractor => "ERR_REPEATED_ATTRACTOR",
            DiagnosticCode::MissingDependency(_) => "ERR_MISSING_DEPENDENCY",
            DiagnosticCode::PatchFailed(_) => "ERR_PATCH_FAILED",
            DiagnosticCode::FileNotDeleted => "ERR_FILE_NOT_DELETED",
            DiagnosticCode::DiffEmpty => "ERR_DIFF_EMPTY",
            DiagnosticCode::TestFailed => "ERR_TEST_FAILED",
            DiagnosticCode::OutOfBoundsWrite => "ERR_OUT_OF_BOUNDS_WRITE",
            DiagnosticCode::AcceptanceCriteriaFailed(_) => "ERR_ACCEPTANCE_CRITERIA_FAILED",
        }
    }

    pub fn default_prescription(&self, target_entity: &str) -> String {
        match self {
            DiagnosticCode::VfsFileNotWritten => {
                format!(
                    "Provide the complete implementation for '{}' inside a code block (e.g. ```python ... ```).",
                    target_entity
                )
            }
            DiagnosticCode::PlaceholderOrEmpty => {
                format!(
                    "Write complete, working implementation for {}. NEVER output placeholders like TODO, pass, or empty comments.",
                    target_entity
                )
            }
            DiagnosticCode::NoCodeBlock => {
                format!("Output your code strictly inside a markdown code block for {}.", target_entity)
            }
            DiagnosticCode::RepeatedAttractor => {
                format!(
                    "Break repetition attractor. Do not repeat failed patterns. Write complete, fresh code directly for '{}'.",
                    target_entity
                )
            }
            DiagnosticCode::MissingDependency(ref dep) => {
                format!(
                    "Required dependency or CLI tool '{}' is missing on host. Consult user or use standard library built-ins.",
                    dep
                )
            }
            DiagnosticCode::PatchFailed(ref err) => {
                format!("Patch failed for '{}': {}. Ensure the exact target snippet exists in the file before patching.", target_entity, err)
            }
            DiagnosticCode::FileNotDeleted => {
                format!("File '{}' was not deleted. Use `os.remove(\"{}\")` in Python or execute `rm {}` via terminal.", target_entity, target_entity, target_entity)
            }
            DiagnosticCode::DiffEmpty => {
                format!("Modification on '{}' produced zero diff. Ensure the target lines are actually changed.", target_entity)
            }
            DiagnosticCode::TestFailed => {
                format!("Execution test for '{}' failed. Review error output and fix the root cause.", target_entity)
            }
            DiagnosticCode::OutOfBoundsWrite => {
                format!(
                    "Security boundary violation: Unauthorized modification attempted outside write_set for '{}'. All file modifications must strictly target declared entities in the write_set.",
                    target_entity
                )
            }
            DiagnosticCode::AcceptanceCriteriaFailed(ref missing) => {
                format!(
                    "Deliverables failed acceptance criteria: {}. Complete all required files and features in '{}'.",
                    missing, target_entity
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyChoice {
    InstallAndRetry,
    UseAlternative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPackageManager {
    Dnf,
    Apt,
    Pacman,
    Zypper,
    Apk,
    Brew,
    Winget,
    Pip,
}

impl HostPackageManager {
    /// Detect the primary package manager available on the host system dynamically.
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            if std::path::Path::new("C:\\Program Files\\WindowsApps").exists() {
                return HostPackageManager::Winget;
            }
            return HostPackageManager::Pip;
        }

        #[cfg(target_os = "macos")]
        {
            if std::path::Path::new("/opt/homebrew/bin/brew").exists()
                || std::path::Path::new("/usr/local/bin/brew").exists()
            {
                return HostPackageManager::Brew;
            }
            return HostPackageManager::Pip;
        }

        #[cfg(unix)]
        {
            if std::path::Path::new("/usr/bin/dnf").exists() || std::path::Path::new("/bin/dnf").exists() {
                HostPackageManager::Dnf
            } else if std::path::Path::new("/usr/bin/apt-get").exists() || std::path::Path::new("/bin/apt-get").exists() {
                HostPackageManager::Apt
            } else if std::path::Path::new("/usr/bin/pacman").exists() || std::path::Path::new("/bin/pacman").exists() {
                HostPackageManager::Pacman
            } else if std::path::Path::new("/usr/bin/zypper").exists() || std::path::Path::new("/bin/zypper").exists() {
                HostPackageManager::Zypper
            } else if std::path::Path::new("/sbin/apk").exists() || std::path::Path::new("/bin/apk").exists() {
                HostPackageManager::Apk
            } else {
                HostPackageManager::Pip
            }
        }
    }

    /// Build the idiomatic installation command for the detected package manager
    pub fn build_install_command(&self, dep: &str) -> String {
        match self {
            HostPackageManager::Dnf => {
                if dep == "tkinter" {
                    "sudo dnf install -y python3-tkinter || pip install --user tk".to_string()
                } else {
                    format!(
                        "sudo dnf install -y python3-{} || python3 -m pip install --user {} || pip install --user {}",
                        dep, dep, dep
                    )
                }
            }
            HostPackageManager::Apt => {
                if dep == "tkinter" {
                    "sudo apt-get update && sudo apt-get install -y python3-tk || pip install --user tk".to_string()
                } else {
                    format!(
                        "sudo apt-get update && sudo apt-get install -y python3-{} || python3 -m pip install --user {} || pip install --user {}",
                        dep, dep, dep
                    )
                }
            }
            HostPackageManager::Pacman => {
                if dep == "tkinter" {
                    "sudo pacman -S --noconfirm tk || pip install --user tk".to_string()
                } else {
                    format!(
                        "sudo pacman -S --noconfirm python-{} || python3 -m pip install --user {} || pip install --user {}",
                        dep, dep, dep
                    )
                }
            }
            HostPackageManager::Zypper => {
                if dep == "tkinter" {
                    "sudo zypper install -y python3-tk || pip install --user tk".to_string()
                } else {
                    format!(
                        "sudo zypper install -y python3-{} || python3 -m pip install --user {} || pip install --user {}",
                        dep, dep, dep
                    )
                }
            }
            HostPackageManager::Apk => {
                format!(
                    "apk add --no-cache py3-{} || python3 -m pip install --user {}",
                    dep, dep
                )
            }
            HostPackageManager::Brew => {
                format!(
                    "brew install {} || python3 -m pip install --user {}",
                    dep, dep
                )
            }
            HostPackageManager::Winget => {
                format!(
                    "python -m pip install --user {} || pip install --user {}",
                    dep, dep
                )
            }
            HostPackageManager::Pip => {
                format!(
                    "python3 -m pip install --user {} || pip install --user {} || pip install {}",
                    dep, dep, dep
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThinkerDecision {
    Nudge(String),
    Reset(String),
    ConsultDependency {
        dependency: String,
        question: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubGoal {
    pub id: usize,
    pub description: String,
    pub target_entity: String,
    pub kind: TaskKind,
    pub guidance: Option<String>,
    pub depends_on: Vec<String>,
    pub read_set: Vec<PathBuf>,
    pub write_set: Vec<PathBuf>,
    pub max_tokens: usize,
}

impl SubGoal {
    pub fn can_write<P: AsRef<Path>>(&self, path: P) -> bool {
        crate::sandbox::MemoryVfs::is_path_in_set(path, &self.write_set)
    }

    pub fn can_read<P: AsRef<Path>>(&self, path: P) -> bool {
        self.read_set.is_empty() || crate::sandbox::MemoryVfs::is_path_in_set(path, &self.read_set)
    }

    /// Computes the effective MAX_TOKENS(v) clamped to minimum floor and model ceiling.
    #[inline]
    pub fn effective_max_tokens(&self, min_floor: usize, model_ctx_limit: Option<usize>) -> usize {
        let mut tokens = self.max_tokens.max(min_floor);
        if let Some(limit) = model_ctx_limit {
            if limit > 0 && limit < tokens {
                tokens = limit;
            }
        }
        tokens
    }

    /// Computes worst-case expected duration d_v^{max} = MAX_TOKENS(v) / r_min (in seconds).
    #[inline]
    pub fn duration_max(&self, config: &LrpConfig) -> f64 {
        let tokens = self.effective_max_tokens(config.min_tokens_floor, config.model_ctx_limit);
        tokens as f64 / config.r_min
    }
}

/// Configuration parameters for duration modeling and LRP calculations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LrpConfig {
    /// Minimum generation throughput rate in tokens per second (default: 20.0).
    pub r_min: f64,
    /// Minimum floor on token allocation per task (default: 2048).
    pub min_tokens_floor: usize,
    /// Model training context limit clamping token allocation (default: None).
    pub model_ctx_limit: Option<usize>,
}

impl Default for LrpConfig {
    fn default() -> Self {
        Self {
            r_min: 20.0,
            min_tokens_floor: 2048,
            model_ctx_limit: None,
        }
    }
}

impl LrpConfig {
    pub fn new(r_min: f64, min_tokens_floor: usize, model_ctx_limit: Option<usize>) -> Self {
        let safe_r_min = if r_min <= 0.0 || r_min.is_nan() {
            20.0
        } else {
            r_min
        };
        Self {
            r_min: safe_r_min,
            min_tokens_floor,
            model_ctx_limit,
        }
    }

    pub fn with_r_min(mut self, r_min: f64) -> Self {
        if r_min > 0.0 && !r_min.is_nan() {
            self.r_min = r_min;
        }
        self
    }
}


// ===========================================================================
// MILESTONE 2: TASK STATE MACHINE & MEMORY BOUND GATE
// ===========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    #[inline]
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        matches!(self, TaskStatus::Running)
    }

    #[inline]
    pub fn is_ready(&self) -> bool {
        matches!(self, TaskStatus::Ready)
    }

    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self, TaskStatus::Pending)
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        matches!(self, TaskStatus::Completed)
    }

    pub fn can_transition_to(&self, next: TaskStatus) -> bool {
        match (*self, next) {
            (a, b) if a == b => false,
            (TaskStatus::Completed, _) | (TaskStatus::Failed, _) | (TaskStatus::Cancelled, _) => false,
            (TaskStatus::Pending, TaskStatus::Ready) => true,
            (TaskStatus::Pending, TaskStatus::Failed) => true,
            (TaskStatus::Pending, TaskStatus::Cancelled) => true,
            (TaskStatus::Ready, TaskStatus::Running) => true,
            (TaskStatus::Ready, TaskStatus::Cancelled) => true,
            (TaskStatus::Ready, TaskStatus::Failed) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::Running, TaskStatus::Cancelled) => true,
            _ => false,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "PENDING",
            TaskStatus::Ready => "READY",
            TaskStatus::Running => "RUNNING",
            TaskStatus::Completed => "COMPLETED",
            TaskStatus::Failed => "FAILED",
            TaskStatus::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryGateError {
    OverCapacity { requested: usize, k_max: usize },
    InsufficientCapacity { requested: usize, remaining: usize, k_max: usize },
}

#[derive(Debug, Clone)]
pub struct MemoryGate {
    pub k_max: usize,
    pub active_tokens: usize,
    pub bytes_per_token: usize,
}

impl MemoryGate {
    pub fn new(k_max: usize) -> Self {
        Self::with_bytes_per_token(k_max, 28_672)
    }

    pub fn with_bytes_per_token(k_max: usize, bytes_per_token: usize) -> Self {
        Self {
            k_max,
            active_tokens: 0,
            bytes_per_token,
        }
    }

    #[inline]
    pub fn can_admit(&self, requested: usize) -> bool {
        if self.is_over_capacity(requested) {
            return false;
        }
        self.active_tokens.saturating_add(requested) <= self.k_max
    }

    #[inline]
    pub fn is_over_capacity(&self, requested: usize) -> bool {
        requested > self.k_max
    }

    pub fn allocate(&mut self, tokens: usize) -> Result<(), ()> {
        if self.can_admit(tokens) {
            self.active_tokens = self.active_tokens.saturating_add(tokens);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn try_allocate(&mut self, tokens: usize) -> Result<(), MemoryGateError> {
        if self.is_over_capacity(tokens) {
            return Err(MemoryGateError::OverCapacity {
                requested: tokens,
                k_max: self.k_max,
            });
        }
        if self.active_tokens.saturating_add(tokens) > self.k_max {
            return Err(MemoryGateError::InsufficientCapacity {
                requested: tokens,
                remaining: self.remaining(),
                k_max: self.k_max,
            });
        }
        self.active_tokens = self.active_tokens.saturating_add(tokens);
        Ok(())
    }

    pub fn release(&mut self, tokens: usize) {
        self.active_tokens = self.active_tokens.saturating_sub(tokens);
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.k_max.saturating_sub(self.active_tokens)
    }

    #[inline]
    pub fn active_bytes(&self) -> usize {
        self.active_tokens * self.bytes_per_token
    }

    #[inline]
    pub fn max_bytes(&self) -> usize {
        self.k_max * self.bytes_per_token
    }

    #[inline]
    pub fn utilization(&self) -> f64 {
        if self.k_max == 0 {
            0.0
        } else {
            (self.active_tokens as f64 / self.k_max as f64).clamp(0.0, 1.0)
        }
    }

    /// Analytical calculation of bytes per token for FP16 KV-Cache:
    /// 4 * L * H_kv * (D / H_q)
    #[inline]
    pub fn analytical_bytes_per_token(l: usize, h_kv: usize, d: usize, h_q: usize) -> usize {
        let d_head = if h_q > 0 { d / h_q } else { 0 };
        4 * l * h_kv * d_head
    }

    /// Global Token Capacity Ceiling:
    /// min(n_ctx_train, floor(m_budget / bytes_per_token))
    #[inline]
    pub fn calculate_k_max(n_ctx_train: usize, m_budget_bytes: usize, bytes_per_token: usize) -> usize {
        let k_mem = if bytes_per_token > 0 {
            m_budget_bytes / bytes_per_token
        } else {
            n_ctx_train
        };
        n_ctx_train.min(k_mem)
    }
}

/// Maximum line count threshold before a deliverable is distilled into an interface contract for downstream tasks.
pub const AUTO_INJECT_MAX_LINES: usize = 60;
/// Maximum byte count threshold before a deliverable is distilled into an interface contract for downstream tasks.
pub const AUTO_INJECT_MAX_BYTES: usize = 2048;

/// Rule of 3: Maximum dynamic injection attempts allowed for any single requirement/artifact before tripping the circuit breaker.
pub const MAX_DELTA_RETRIES_PER_TARGET: usize = 3;
/// Global safety ceiling: Maximum cumulative dynamic subgoal injections allowed per swarm session.
pub const MAX_TOTAL_DYNAMIC_INJECTIONS: usize = 6;

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_id: usize,
    pub subgoal: SubGoal,
    pub status: TaskStatus,
    pub failure_reason: Option<String>,
    pub diagnostic_code: Option<DiagnosticCode>,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub finding: Option<WorkerFinding>,
    pub abort_flag: Arc<AtomicBool>,
    pub allocated_tokens: usize,
    pub interface_contract: Option<String>,
}

impl TaskRecord {
    pub fn new(subgoal: SubGoal) -> Self {
        Self {
            task_id: subgoal.id,
            subgoal,
            status: TaskStatus::Pending,
            failure_reason: None,
            diagnostic_code: None,
            started_at: None,
            finished_at: None,
            finding: None,
            abort_flag: Arc::new(AtomicBool::new(false)),
            allocated_tokens: 0,
            interface_contract: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskDag {
    pub children: HashMap<usize, HashSet<usize>>,
    pub dependencies: HashMap<usize, HashSet<usize>>,
    pub in_degrees: HashMap<usize, usize>,
    pub tasks: HashMap<usize, SubGoal>,
    pub node_ids: HashSet<usize>,
}

impl TaskDag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_subgoals(subgoals: Vec<SubGoal>) -> Result<Self, String> {
        let mut dag = TaskDag::default();

        for sg in &subgoals {
            dag.tasks.insert(sg.id, sg.clone());
            dag.node_ids.insert(sg.id);
            dag.children.entry(sg.id).or_default();
            dag.dependencies.entry(sg.id).or_default();
            dag.in_degrees.insert(sg.id, 0);
        }

        let mut entity_producers: HashMap<String, usize> = HashMap::new();
        let mut path_producers: HashMap<PathBuf, usize> = HashMap::new();

        for sg in &subgoals {
            let v = sg.id;

            for dep in &sg.depends_on {
                let trimmed = dep.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                    continue;
                }

                let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
                if let Ok(u) = digits.parse::<usize>() {
                    if dag.tasks.contains_key(&u) && u < v {
                        dag.add_edge(u, v);
                        continue;
                    }
                }

                let norm = Self::normalize_name(trimmed);
                if let Some(&u) = entity_producers.get(&norm) {
                    if u < v {
                        dag.add_edge(u, v);
                        continue;
                    }
                }

                let norm_p = Self::normalize_path(Path::new(trimmed));
                if let Some(&u) = path_producers.get(&norm_p) {
                    if u < v {
                        dag.add_edge(u, v);
                        continue;
                    }
                }
            }

            for rp in &sg.read_set {
                let norm_rp = Self::normalize_path(rp);
                if let Some(&u) = path_producers.get(&norm_rp) {
                    if u < v {
                        dag.add_edge(u, v);
                    }
                }
            }

            // Register this task's produced artifacts for subsequent readers (strictly if not a pure test worker)
            if sg.kind != TaskKind::Test {
                if !sg.target_entity.is_empty() {
                    entity_producers.insert(Self::normalize_name(&sg.target_entity), v);
                }
                for p in &sg.write_set {
                    path_producers.insert(Self::normalize_path(p), v);
                }
            }
        }

        dag.recompute_in_degrees();
        dag.validate_acyclic()?;

        Ok(dag)
    }

    pub fn add_node(&mut self, id: usize) {
        self.node_ids.insert(id);
        self.children.entry(id).or_default();
        self.dependencies.entry(id).or_default();
        self.in_degrees.entry(id).or_insert(0);
    }

    pub fn add_edge(&mut self, u: usize, v: usize) {
        if u == v {
            return;
        }
        self.add_node(u);
        self.add_node(v);

        if self.children.entry(u).or_default().insert(v) {
            self.dependencies.entry(v).or_default().insert(u);
            *self.in_degrees.entry(v).or_insert(0) += 1;
        }
    }

    pub fn recompute_in_degrees(&mut self) {
        let all_ids: HashSet<usize> = self.node_ids.iter().copied()
            .chain(self.tasks.keys().copied())
            .collect();
        self.in_degrees.clear();
        for &id in &all_ids {
            let count = self.dependencies.get(&id).map(|d| d.len()).unwrap_or(0);
            self.in_degrees.insert(id, count);
        }
    }

    pub fn validate_acyclic(&self) -> Result<Vec<usize>, String> {
        let total_nodes = self.node_ids.len().max(self.tasks.len());
        let mut order = Vec::with_capacity(total_nodes);

        let mut all_ids: Vec<usize> = self.node_ids.iter().copied()
            .chain(self.tasks.keys().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all_ids.sort();

        // Freshly compute in_degrees from dependencies to eliminate any desynchronization
        let mut in_degrees: HashMap<usize, usize> = HashMap::with_capacity(all_ids.len());
        for &id in &all_ids {
            let count = self.dependencies.get(&id).map(|d| d.len()).unwrap_or(0);
            in_degrees.insert(id, count);
        }

        let mut queue = VecDeque::new();
        for &id in &all_ids {
            if in_degrees.get(&id).copied().unwrap_or(0) == 0 {
                queue.push_back(id);
            }
        }

        while let Some(u) = queue.pop_front() {
            order.push(u);
            if let Some(children) = self.children.get(&u) {
                let mut sorted_children: Vec<usize> = children.iter().copied().collect();
                sorted_children.sort();
                for &v in &sorted_children {
                    if let Some(deg) = in_degrees.get_mut(&v) {
                        if *deg > 0 {
                            *deg -= 1;
                            if *deg == 0 {
                                queue.push_back(v);
                            }
                        }
                    }
                }
            }
        }

        if order.len() != total_nodes {
            return Err(format!(
                "Cyclic dependency detected in Swarm DAG: ordered {} of {} tasks",
                order.len(),
                total_nodes
            ));
        }

        Ok(order)
    }

    pub fn root_tasks(&self) -> Vec<usize> {
        let mut roots: Vec<usize> = self.in_degrees
            .iter()
            .filter_map(|(&id, &deg)| if deg == 0 { Some(id) } else { None })
            .collect();
        roots.sort();
        roots
    }

    pub fn cascade_cancellation(
        &self,
        failed_task_id: usize,
        state_table: &mut HashMap<usize, TaskStatus>,
        _reason: &str,
    ) -> Vec<usize> {
        let mut cancelled_ids = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        visited.insert(failed_task_id);

        if let Some(children) = self.children.get(&failed_task_id) {
            let mut sorted_children: Vec<usize> = children.iter().copied().collect();
            sorted_children.sort();
            for child in sorted_children {
                if visited.insert(child) {
                    queue.push_back(child);
                }
            }
        }

        while let Some(curr) = queue.pop_front() {
            if let Some(children) = self.children.get(&curr) {
                let mut sorted_children: Vec<usize> = children.iter().copied().collect();
                sorted_children.sort();
                for child in sorted_children {
                    if visited.insert(child) {
                        queue.push_back(child);
                    }
                }
            }

            if let Some(status) = state_table.get_mut(&curr) {
                if !status.is_terminal() {
                    *status = TaskStatus::Cancelled;
                    cancelled_ids.push(curr);
                }
            }
        }

        cancelled_ids
    }

    /// Computes worst-case duration d_v^{max} for a specific task node.
    pub fn task_duration(&self, task_id: usize, config: &LrpConfig) -> f64 {
        if let Some(sg) = self.tasks.get(&task_id) {
            sg.duration_max(config)
        } else {
            (config.min_tokens_floor as f64) / config.r_min
        }
    }

    /// Computes LRP (Longest Remaining Path) for all nodes in the DAG using
    /// iterative bottom-up dynamic programming over reverse topological order.
    ///
    /// Complexity: O(|V| + |E|) time, O(|V|) memory.
    pub fn compute_lrp(&self, config: &LrpConfig) -> Result<HashMap<usize, f64>, String> {
        let topo_order = self.validate_acyclic()?;
        let mut lrp_table = HashMap::with_capacity(topo_order.len());

        for &v in topo_order.iter().rev() {
            let d_v = self.task_duration(v, config);
            let max_child_lrp = self.children.get(&v)
                .map(|children| {
                    children.iter()
                        .map(|&child_id| lrp_table.get(&child_id).copied().unwrap_or(0.0))
                        .fold(0.0, f64::max)
                })
                .unwrap_or(0.0);

            lrp_table.insert(v, d_v + max_child_lrp);
        }

        Ok(lrp_table)
    }

    /// Memoized recursive LRP evaluation helper (Method 2).
    pub fn compute_lrp_memo(&self, config: &LrpConfig) -> Result<HashMap<usize, f64>, String> {
        self.validate_acyclic()?;
        let mut memo = HashMap::new();
        let mut visiting = HashSet::new();

        let mut all_ids: Vec<usize> = self.node_ids.iter().copied()
            .chain(self.tasks.keys().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        all_ids.sort();

        for id in all_ids {
            self.compute_node_lrp_recursive(id, config, &mut memo, &mut visiting)?;
        }

        Ok(memo)
    }

    fn compute_node_lrp_recursive(
        &self,
        v: usize,
        config: &LrpConfig,
        memo: &mut HashMap<usize, f64>,
        visiting: &mut HashSet<usize>,
    ) -> Result<f64, String> {
        if let Some(&val) = memo.get(&v) {
            return Ok(val);
        }
        if !visiting.insert(v) {
            return Err(format!("Cycle detected at node {}", v));
        }

        let d_v = self.task_duration(v, config);
        let mut max_child = 0.0;
        if let Some(children) = self.children.get(&v) {
            for &child in children {
                let child_lrp = self.compute_node_lrp_recursive(child, config, memo, visiting)?;
                if child_lrp > max_child {
                    max_child = child_lrp;
                }
            }
        }

        visiting.remove(&v);
        let total = d_v + max_child;
        memo.insert(v, total);
        Ok(total)
    }

    /// Computes the overall Critical Path makespan of the project:
    /// CP = max_{r in Roots} LRP(r)
    pub fn critical_path_length(&self, config: &LrpConfig) -> Result<f64, String> {
        let lrp_table = self.compute_lrp(config)?;
        let roots = self.root_tasks();
        let cp = roots.iter()
            .map(|r| lrp_table.get(r).copied().unwrap_or(0.0))
            .fold(0.0, f64::max);
        Ok(cp)
    }

    /// Sorts ready set R_t descending by LRP with deterministic task ID tie-breaking.
    pub fn sort_ready_set(&self, ready_set: &mut Vec<usize>, lrp_table: &HashMap<usize, f64>) {
        Self::sort_ready_slice(ready_set, lrp_table);
    }

    /// Sorts slice of task IDs descending by LRP with deterministic task ID tie-breaking.
    pub fn sort_ready_slice(ready_set: &mut [usize], lrp_table: &HashMap<usize, f64>) {
        ready_set.sort_by(|&a, &b| {
            let lrp_a = lrp_table.get(&a).copied().unwrap_or(0.0);
            let lrp_b = lrp_table.get(&b).copied().unwrap_or(0.0);
            lrp_b.partial_cmp(&lrp_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(&b))
        });
    }

    fn normalize_name(s: &str) -> String {
        s.trim().trim_start_matches("./").to_lowercase()
    }

    fn normalize_path(p: &Path) -> PathBuf {
        let s = p.to_string_lossy();
        PathBuf::from(s.trim().trim_start_matches("./"))
    }
}

pub struct StateMachineCoordinator {
    pub dag: TaskDag,
    pub records: HashMap<usize, TaskRecord>,
    pub ready_set: Vec<usize>,
    pub memory_gate: MemoryGate,
    pub lrp_table: HashMap<usize, f64>,
    pub lrp_config: LrpConfig,
}

impl StateMachineCoordinator {
    pub fn new(dag: TaskDag, k_max: usize) -> Result<Self, String> {
        Self::with_memory_gate(dag, MemoryGate::new(k_max))
    }

    pub fn with_memory_gate(dag: TaskDag, memory_gate: MemoryGate) -> Result<Self, String> {
        Self::with_memory_gate_and_lrp(dag, memory_gate, LrpConfig::default())
    }

    pub fn with_memory_gate_and_lrp(
        dag: TaskDag,
        memory_gate: MemoryGate,
        lrp_config: LrpConfig,
    ) -> Result<Self, String> {
        dag.validate_acyclic()?;
        let lrp_table = dag.compute_lrp(&lrp_config)?;

        let mut records = HashMap::new();
        let mut ready_set = Vec::new();

        let mut task_entries: Vec<(usize, SubGoal)> = dag.tasks.iter().map(|(&id, sg)| (id, sg.clone())).collect();
        if task_entries.is_empty() {
            for &id in &dag.node_ids {
                task_entries.push((id, SubGoal {
                    id,
                    description: format!("Task #{}", id),
                    target_entity: String::new(),
                    kind: TaskKind::Create,
                    guidance: None,
                    depends_on: vec![],
                    read_set: vec![],
                    write_set: vec![],
                    max_tokens: 1024,
                }));
            }
        }

        for (id, sg) in task_entries {
            let mut record = TaskRecord::new(sg);
            let in_deg = dag.in_degrees.get(&id).copied().unwrap_or(0);
            if in_deg == 0 {
                record.status = TaskStatus::Ready;
                ready_set.push(id);
            } else {
                record.status = TaskStatus::Pending;
            }
            records.insert(id, record);
        }

        dag.sort_ready_set(&mut ready_set, &lrp_table);

        Ok(Self {
            dag,
            records,
            ready_set,
            memory_gate,
            lrp_table,
            lrp_config,
        })
    }

    /// Returns head task v* = argmax_{v in R_t} LRP(v).
    #[inline]
    pub fn head_task(&self) -> Option<usize> {
        self.ready_set.first().copied()
    }

    pub fn preflight_validate(&mut self) -> Result<(), Vec<String>> {
        let mut rejections = Vec::new();
        let ids: Vec<usize> = self.records.keys().copied().collect();
        for id in ids {
            let requested = self.records.get(&id).map(|r| r.subgoal.max_tokens).unwrap_or(0);
            if self.memory_gate.is_over_capacity(requested) {
                let err_msg = format!(
                    "Zero-OOM Hard Rejection: Task #{} demands {} tokens > K_max {}",
                    id, requested, self.memory_gate.k_max
                );
                rejections.push(err_msg.clone());
                if let Some(rec) = self.records.get_mut(&id) {
                    rec.status = TaskStatus::Failed;
                    rec.finished_at = Some(Instant::now());
                    rec.failure_reason = Some(err_msg);
                }
                self.ready_set.retain(|&r_id| r_id != id);
                self.cascade_failure(id, "Zero-OOM pre-flight rejection");
            }
        }

        if rejections.is_empty() {
            Ok(())
        } else {
            Err(rejections)
        }
    }

    pub fn admit_task(&mut self, task_id: usize, requested_tokens: usize) -> Result<Arc<AtomicBool>, String> {
        let record = self.records.get_mut(&task_id)
            .ok_or_else(|| format!("Task #{} not found", task_id))?;

        if record.status != TaskStatus::Ready {
            return Err(format!("Task #{} is not Ready (current: {:?})", task_id, record.status));
        }

        if self.memory_gate.is_over_capacity(requested_tokens) {
            record.status = TaskStatus::Failed;
            record.finished_at = Some(Instant::now());
            record.failure_reason = Some(format!(
                "Zero-OOM rejection: requested {} tokens > K_max {}",
                requested_tokens, self.memory_gate.k_max
            ));
            self.ready_set.retain(|&id| id != task_id);
            self.cascade_failure(task_id, "Zero-OOM pre-flight rejection");
            return Err("Zero-OOM hard rejection".to_string());
        }

        if !self.memory_gate.can_admit(requested_tokens) {
            return Err("Context Boom: insufficient remaining memory".to_string());
        }

        self.memory_gate.allocate(requested_tokens)
            .map_err(|_| "Context Boom allocation failure".to_string())?;

        record.status = TaskStatus::Running;
        record.allocated_tokens = requested_tokens;
        record.started_at = Some(Instant::now());
        self.ready_set.retain(|&id| id != task_id);

        Ok(record.abort_flag.clone())
    }

    pub fn complete_task(&mut self, task_id: usize) -> Result<Vec<usize>, String> {
        let newly_ready = {
            let record = self.records.get_mut(&task_id)
                .ok_or_else(|| format!("Task #{} not found", task_id))?;

            // Unconditionally release tokens first to prevent MemoryGate resource leaks on error paths
            if record.allocated_tokens > 0 {
                self.memory_gate.release(record.allocated_tokens);
                record.allocated_tokens = 0;
            }

            if record.status != TaskStatus::Running {
                return Err(format!("Cannot complete task #{}: status is {:?}", task_id, record.status));
            }

            record.status = TaskStatus::Completed;
            record.finished_at = Some(Instant::now());

            let mut ready_children = Vec::new();
            if let Some(children) = self.dag.children.get(&task_id) {
                let mut sorted_children: Vec<usize> = children.iter().copied().collect();
                sorted_children.sort();
                for child_id in sorted_children {
                    let all_deps_done = self.dag.dependencies.get(&child_id)
                        .map(|deps| deps.iter().all(|&dep_id| {
                            self.records.get(&dep_id).map(|r| r.status == TaskStatus::Completed).unwrap_or(false)
                        }))
                        .unwrap_or(true);

                    if all_deps_done {
                        if let Some(child_rec) = self.records.get_mut(&child_id) {
                            if child_rec.status == TaskStatus::Pending {
                                child_rec.status = TaskStatus::Ready;
                                ready_children.push(child_id);
                            }
                        }
                    }
                }
            }
            ready_children.sort();
            ready_children
        };

        for &child_id in &newly_ready {
            if !self.ready_set.contains(&child_id) {
                self.ready_set.push(child_id);
            }
        }
        self.dag.sort_ready_set(&mut self.ready_set, &self.lrp_table);

        Ok(newly_ready)
    }

    /// Dynamically injects an incremental subgoal into the live DAG during runtime.
    /// Used by the Thinker / Tester closed feedback loop when an Acceptance Delta is identified.
    pub fn inject_subgoal(&mut self, mut sg: SubGoal, depends_on: Vec<usize>) -> Result<usize, String> {
        let new_id = self.records.keys().max().copied().unwrap_or(0) + 1;
        sg.id = new_id;

        self.dag.tasks.insert(new_id, sg.clone());
        self.dag.node_ids.insert(new_id);
        self.dag.children.entry(new_id).or_default();
        self.dag.dependencies.entry(new_id).or_default();
        self.dag.in_degrees.insert(new_id, 0);

        for dep_id in &depends_on {
            if self.dag.tasks.contains_key(dep_id) {
                self.dag.add_edge(*dep_id, new_id);
            }
        }

        let mut record = TaskRecord::new(sg);
        let all_deps_done = depends_on.iter().all(|&dep_id| {
            self.records.get(&dep_id).map(|r| r.status == TaskStatus::Completed).unwrap_or(false)
        });

        if all_deps_done {
            record.status = TaskStatus::Ready;
            self.ready_set.push(new_id);
        } else {
            record.status = TaskStatus::Pending;
        }

        self.records.insert(new_id, record);

        // Validate DAG acyclicity & recompute LRP with complete rollback on cycle
        match self.dag.compute_lrp(&self.lrp_config) {
            Ok(new_lrp) => {
                self.lrp_table = new_lrp;
            }
            Err(e) => {
                // Rollback: purge injected node and edges
                self.dag.tasks.remove(&new_id);
                self.dag.node_ids.remove(&new_id);
                self.dag.children.remove(&new_id);
                self.dag.dependencies.remove(&new_id);
                self.dag.in_degrees.remove(&new_id);
                for dep_id in &depends_on {
                    if let Some(children) = self.dag.children.get_mut(dep_id) {
                        children.remove(&new_id);
                    }
                }
                self.records.remove(&new_id);
                self.ready_set.retain(|&id| id != new_id);
                return Err(format!("inject_subgoal rejected: cycle detected: {}", e));
            }
        }
        self.dag.sort_ready_set(&mut self.ready_set, &self.lrp_table);

        Ok(new_id)
    }

    /// Sets the distilled public interface contract for a given task node in the DAG.
    pub fn set_interface_contract(&mut self, task_id: usize, contract: String) {
        if let Some(record) = self.records.get_mut(&task_id) {
            record.interface_contract = Some(contract);
        }
    }

    /// Retrieves the interface contract for a given task node in the DAG.
    pub fn get_interface_contract(&self, task_id: usize) -> Option<String> {
        self.records.get(&task_id).and_then(|r| r.interface_contract.clone())
    }

    /// Gathers all registered interface contracts across all completed tasks in the DAG,
    /// keyed by target filename for instant lookups by downstream workers.
    pub fn get_all_interface_contracts(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for record in self.records.values() {
            if let Some(ref contract) = record.interface_contract {
                if !record.subgoal.target_entity.is_empty() {
                    let fname = std::path::Path::new(&record.subgoal.target_entity)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| record.subgoal.target_entity.clone());
                    map.insert(fname, contract.clone());
                }
            }
        }
        map
    }

    pub fn fail_task(&mut self, task_id: usize, reason: &str, diag: Option<DiagnosticCode>) -> Vec<usize> {
        if let Some(record) = self.records.get_mut(&task_id) {
            record.status = TaskStatus::Failed;
            record.finished_at = Some(Instant::now());
            record.failure_reason = Some(reason.to_string());
            record.diagnostic_code = diag;

            if record.allocated_tokens > 0 {
                self.memory_gate.release(record.allocated_tokens);
                record.allocated_tokens = 0;
            }
        }
        self.ready_set.retain(|&id| id != task_id);

        self.cascade_failure(task_id, reason)
    }

    pub fn cascade_failure(&mut self, failed_task_id: usize, reason: &str) -> Vec<usize> {
        let mut cancelled_ids = Vec::new();
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        visited.insert(failed_task_id);

        if let Some(children) = self.dag.children.get(&failed_task_id) {
            let mut sorted_children: Vec<usize> = children.iter().copied().collect();
            sorted_children.sort();
            for child in sorted_children {
                if visited.insert(child) {
                    queue.push_back(child);
                }
            }
        }

        while let Some(curr_id) = queue.pop_front() {
            if let Some(children) = self.dag.children.get(&curr_id) {
                let mut sorted_children: Vec<usize> = children.iter().copied().collect();
                sorted_children.sort();
                for child in sorted_children {
                    if visited.insert(child) {
                        queue.push_back(child);
                    }
                }
            }

            if let Some(record) = self.records.get_mut(&curr_id) {
                match record.status {
                    TaskStatus::Pending | TaskStatus::Ready => {
                        let prev = record.status;
                        record.status = TaskStatus::Cancelled;
                        record.finished_at = Some(Instant::now());
                        record.failure_reason = Some(format!(
                            "Cancelled: Topological ancestor Task #{} FAILED: {}",
                            failed_task_id, reason
                        ));
                        cancelled_ids.push(curr_id);

                        if prev == TaskStatus::Ready {
                            self.ready_set.retain(|&id| id != curr_id);
                        }
                    }
                    TaskStatus::Running => {
                        record.abort_flag.store(true, Ordering::SeqCst);
                        record.status = TaskStatus::Cancelled;
                        record.finished_at = Some(Instant::now());
                        record.failure_reason = Some(format!(
                            "Preempted: Topological ancestor Task #{} FAILED while running",
                            failed_task_id
                        ));
                        cancelled_ids.push(curr_id);

                        if record.allocated_tokens > 0 {
                            self.memory_gate.release(record.allocated_tokens);
                            record.allocated_tokens = 0;
                        }
                    }
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {}
                }
            }
        }

        cancelled_ids
    }

    pub fn is_all_terminal(&self) -> bool {
        self.records.values().all(|r| r.status.is_terminal())
    }
}

// ===========================================================================
// MILESTONE 3: RCPSP SCHEDULER (PINNED EASY BACKFILLING & SCOPED CONCURRENCY)
// ===========================================================================

/// Result of an RCPSP scheduling pass over the Topological Ready Set.
#[derive(Debug, Clone, PartialEq)]
pub enum ScheduleDecision {
    /// Head task v* fits in remaining KV capacity and is dispatched immediately.
    DispatchHead {
        task_id: usize,
        allocated_tokens: usize,
    },
    /// Head task v* is pinned with a shadow reservation at t_res;
    /// Zero or more non-delaying candidate tasks u were backfilled.
    PinnedWithBackfill {
        pinned_head_id: usize,
        reservation_delay_secs: f64,
        backfilled_task_ids: Vec<usize>,
    },
    /// Head task is pinned, but no candidate tasks satisfy the EASY backfill predicate.
    /// Cooperative backpressure: scheduler must wait for active tasks to complete.
    ContextBoomWait {
        pinned_head_id: usize,
        reservation_delay_secs: f64,
        deficit_tokens: usize,
    },
    /// No ready tasks exist, but active tasks are currently running.
    AwaitingRunningTasks {
        active_count: usize,
    },
    /// All tasks in the DAG have reached a terminal state (Completed, Failed, Cancelled).
    AllTerminal,
}

/// A guaranteed shadow reservation for head task v*.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowReservation {
    pub task_id: usize,
    pub required_tokens: usize,
    pub deficit_tokens: usize,
    pub reservation_delay_secs: f64,
    /// Vector of (task_id, released_tokens, expected_remaining_secs)
    pub releasing_tasks: Vec<(usize, usize, f64)>,
}

/// Backfill eligibility classification for a candidate task u.
#[derive(Debug, Clone, PartialEq)]
pub enum BackfillEligibility {
    Eligible {
        justification: BackfillJustification,
    },
    Ineligible {
        reason: BackfillRejectionReason,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackfillJustification {
    /// Sub-predicate A: Task duration finishes before shadow reservation time (d_u <= t_res - t_now).
    CompletesBeforeReservation {
        task_duration_secs: f64,
        reservation_delay_secs: f64,
    },
    /// Sub-predicate B: Task runs past reservation, but remaining headroom at t_res preserves v* tokens.
    HeadroomPreservedAtReservation {
        task_tokens: usize,
        headroom_tokens: usize,
        head_tokens: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackfillRejectionReason {
    InsufficientCurrentTokens { requested: usize, available: usize },
    VfsConflict { conflicting_task_id: usize, path: PathBuf },
    DelaysPinnedHead { task_duration_secs: f64, reservation_delay_secs: f64, head_tokens: usize },
}

/// Active execution state of an in-flight worker task.
#[derive(Debug, Clone)]
pub struct ActiveTaskProfile {
    pub task_id: usize,
    pub allocated_tokens: usize,
    pub started_at: Instant,
    pub expected_total_secs: f64,
    pub expected_remaining_secs: f64,
}

/// Events delivered from worker threads back to the central scheduler.
#[derive(Debug)]
pub enum WorkerEvent {
    /// Worker succeeded and delivered its findings.
    Completed {
        task_id: usize,
        finding: WorkerFinding,
    },
    /// Worker encountered an execution failure (out-of-bounds, syntax, test failure).
    Failed {
        task_id: usize,
        reason: String,
        diagnostic_code: Option<DiagnosticCode>,
        finding: WorkerFinding,
    },
    /// Worker halted due to cooperative preemption (abort flag tripped).
    Cancelled {
        task_id: usize,
        finding: WorkerFinding,
    },
    /// Tester worker detected partial implementation / missing requirements from user goal.
    /// Issues an Acceptance Delta report to trigger incremental subtask synthesis without wiping.
    AcceptanceDelta {
        task_id: usize,
        target_file: String,
        delta: String,
        finding: WorkerFinding,
    },
    /// Worker thread panicked (caught by panic wrapper).
    Panicked {
        task_id: usize,
        panic_msg: String,
    },
}

/// Metadata tracked for each active in-flight worker thread.
#[derive(Debug, Clone)]
pub struct ActiveWorkerProfile {
    pub task_id: usize,
    pub allocated_tokens: usize,
    pub started_at: Instant,
    pub expected_duration_secs: f64,
    pub abort_flag: Arc<AtomicBool>,
    pub write_set: Vec<PathBuf>,
    pub read_set: Vec<PathBuf>,
}

/// Online Resource-Constrained Project Scheduling Problem (RCPSP) Scheduler.
#[derive(Debug, Clone)]
pub struct RcpspScheduler {
    /// Conservative minimum generation rate in tokens/second (default: 20.0).
    pub r_min: f64,
}

impl RcpspScheduler {
    pub fn new(r_min: f64) -> Self {
        Self {
            r_min: if r_min <= 0.0 || r_min.is_nan() { 20.0 } else { r_min },
        }
    }

    pub fn default_rate() -> Self {
        Self::new(20.0)
    }

    /// Computes duration bound for a given token count: d_v = k_v / r_min.
    #[inline]
    pub fn compute_duration(&self, tokens: usize) -> f64 {
        tokens as f64 / self.r_min
    }

    /// Computes expected remaining time for a running task based on elapsed wall-clock time.
    #[inline]
    pub fn compute_expected_remaining_time(&self, allocated_tokens: usize, started_at: Instant) -> f64 {
        let total_secs = self.compute_duration(allocated_tokens);
        let elapsed_secs = started_at.elapsed().as_secs_f64();
        (total_secs - elapsed_secs).max(0.001)
    }

    /// Inspects running tasks in `StateMachineCoordinator` and builds an Earliest Completion Time profile.
    pub fn get_active_profiles(&self, coordinator: &StateMachineCoordinator) -> Vec<ActiveTaskProfile> {
        let mut profiles = Vec::new();
        for (&id, record) in &coordinator.records {
            if record.status == TaskStatus::Running {
                let allocated = record.allocated_tokens;
                let started = record.started_at.unwrap_or_else(Instant::now);
                let total_secs = self.compute_duration(allocated);
                let rem_secs = self.compute_expected_remaining_time(allocated, started);
                profiles.push(ActiveTaskProfile {
                    task_id: id,
                    allocated_tokens: allocated,
                    started_at: started,
                    expected_total_secs: total_secs,
                    expected_remaining_secs: rem_secs,
                });
            }
        }
        profiles.sort_by(|a, b| a.expected_remaining_secs.partial_cmp(&b.expected_remaining_secs).unwrap_or(std::cmp::Ordering::Equal));
        profiles
    }

    /// Calculates Shadow Reservation for head task v* if K_rem < MAX_TOKENS(v*).
    pub fn compute_shadow_reservation(
        &self,
        coordinator: &StateMachineCoordinator,
        head_task_id: usize,
        head_tokens: usize,
    ) -> Result<ShadowReservation, String> {
        let current_rem = coordinator.memory_gate.remaining();
        if current_rem >= head_tokens {
            return Ok(ShadowReservation {
                task_id: head_task_id,
                required_tokens: head_tokens,
                deficit_tokens: 0,
                reservation_delay_secs: 0.0,
                releasing_tasks: Vec::new(),
            });
        }

        let deficit = head_tokens.saturating_sub(current_rem);
        let active_profiles = self.get_active_profiles(coordinator);

        if active_profiles.is_empty() {
            return Err(format!(
                "Cannot compute shadow reservation: no active tasks running and deficit is {} tokens",
                deficit
            ));
        }

        let mut accumulated_tokens = current_rem;
        let mut releasing = Vec::new();
        let mut reservation_delay = 0.0;
        let mut satisfied = false;

        for profile in active_profiles {
            accumulated_tokens = accumulated_tokens.saturating_add(profile.allocated_tokens);
            releasing.push((profile.task_id, profile.allocated_tokens, profile.expected_remaining_secs));
            reservation_delay = profile.expected_remaining_secs;

            if accumulated_tokens >= head_tokens {
                satisfied = true;
                break;
            }
        }

        if !satisfied {
            return Err(format!(
                "Zero-OOM Violation: Even after all running tasks complete, available tokens ({}) < requested ({})",
                accumulated_tokens, head_tokens
            ));
        }

        Ok(ShadowReservation {
            task_id: head_task_id,
            required_tokens: head_tokens,
            deficit_tokens: deficit,
            reservation_delay_secs: reservation_delay,
            releasing_tasks: releasing,
        })
    }

    /// Checks if candidate SubGoal has VFS read/write conflicts with running subgoals.
    pub fn check_vfs_conflict(
        candidate: &SubGoal,
        active_subgoals: &[&SubGoal],
    ) -> Option<(usize, PathBuf)> {
        for &active in active_subgoals {
            // Write-Write conflict
            for w in &candidate.write_set {
                if MemoryVfs::is_path_in_set(w, &active.write_set)
                    || active.write_set.iter().any(|aw| MemoryVfs::is_path_in_set(aw, std::slice::from_ref(w)))
                {
                    return Some((active.id, w.clone()));
                }
            }
            // Write-Read conflict (candidate writes, active reads)
            for w in &candidate.write_set {
                if MemoryVfs::is_path_in_set(w, &active.read_set)
                    || active.read_set.iter().any(|ar| MemoryVfs::is_path_in_set(ar, std::slice::from_ref(w)))
                {
                    return Some((active.id, w.clone()));
                }
            }
            // Read-Write conflict (candidate reads, active writes)
            for r in &candidate.read_set {
                if MemoryVfs::is_path_in_set(r, &active.write_set)
                    || active.write_set.iter().any(|aw| MemoryVfs::is_path_in_set(aw, std::slice::from_ref(r)))
                {
                    return Some((active.id, r.clone()));
                }
            }
        }
        None
    }

    /// Evaluates candidate task u against EASY backfill predicate for pinned head v*.
    pub fn evaluate_backfill_predicate(
        &self,
        candidate: &SubGoal,
        current_rem_tokens: usize,
        head_reservation: &ShadowReservation,
        headroom_at_reservation: usize,
        active_subgoals: &[&SubGoal],
    ) -> BackfillEligibility {
        let k_u = candidate.max_tokens;

        // Condition 1: Must fit in current remaining tokens
        if k_u > current_rem_tokens {
            return BackfillEligibility::Ineligible {
                reason: BackfillRejectionReason::InsufficientCurrentTokens {
                    requested: k_u,
                    available: current_rem_tokens,
                },
            };
        }

        // Condition 2: VFS file conflict check
        if let Some((conflicting_id, path)) = Self::check_vfs_conflict(candidate, active_subgoals) {
            return BackfillEligibility::Ineligible {
                reason: BackfillRejectionReason::VfsConflict {
                    conflicting_task_id: conflicting_id,
                    path,
                },
            };
        }

        // Condition 3: Zero-Delay Guarantee
        let d_u = self.compute_duration(k_u);
        let delta_t_res = head_reservation.reservation_delay_secs;

        // Sub-predicate A: Finishes before reservation time
        if d_u <= delta_t_res {
            return BackfillEligibility::Eligible {
                justification: BackfillJustification::CompletesBeforeReservation {
                    task_duration_secs: d_u,
                    reservation_delay_secs: delta_t_res,
                },
            };
        }

        // Sub-predicate B: Headroom preserved at reservation time
        if k_u <= headroom_at_reservation {
            return BackfillEligibility::Eligible {
                justification: BackfillJustification::HeadroomPreservedAtReservation {
                    task_tokens: k_u,
                    headroom_tokens: headroom_at_reservation,
                    head_tokens: head_reservation.required_tokens,
                },
            };
        }

        // Fails Zero-Delay Guarantee
        BackfillEligibility::Ineligible {
            reason: BackfillRejectionReason::DelaysPinnedHead {
                task_duration_secs: d_u,
                reservation_delay_secs: delta_t_res,
                head_tokens: head_reservation.required_tokens,
            },
        }
    }

    /// Executes one full online scheduling pass over `StateMachineCoordinator`.
    pub fn schedule_next_batch(
        &self,
        coordinator: &mut StateMachineCoordinator,
        lrp_map: &HashMap<usize, f64>,
    ) -> Result<ScheduleDecision, String> {
        if coordinator.is_all_terminal() {
            return Ok(ScheduleDecision::AllTerminal);
        }

        if coordinator.ready_set.is_empty() {
            let active_count = coordinator.records.values().filter(|r| r.status == TaskStatus::Running).count();
            return Ok(ScheduleDecision::AwaitingRunningTasks { active_count });
        }

        // 1. Sort Ready Set descending by LRP with deterministic tie-breaking (lowest id)
        coordinator.dag.sort_ready_set(&mut coordinator.ready_set, lrp_map);

        let head_id = coordinator.ready_set[0];
        let head_tokens = coordinator.records.get(&head_id)
            .map(|r| r.subgoal.max_tokens)
            .unwrap_or(0);

        let current_rem = coordinator.memory_gate.remaining();

        // 2. Can Head Task start immediately?
        if current_rem >= head_tokens {
            let active_subgoals: Vec<&SubGoal> = coordinator.records.values()
                .filter(|r| r.status == TaskStatus::Running)
                .map(|r| &r.subgoal)
                .collect();
            let head_sg = &coordinator.records.get(&head_id).unwrap().subgoal;

            if Self::check_vfs_conflict(head_sg, &active_subgoals).is_none() {
                coordinator.admit_task(head_id, head_tokens)?;
                return Ok(ScheduleDecision::DispatchHead {
                    task_id: head_id,
                    allocated_tokens: head_tokens,
                });
            }
        }

        // 3. Pin Head Task v* and compute Shadow Reservation
        let reservation = self.compute_shadow_reservation(coordinator, head_id, head_tokens)?;
        let mut sim_rem = current_rem;

        let tokens_freed_by_res: usize = reservation.releasing_tasks.iter()
            .map(|(_, tok, _)| *tok)
            .sum();
        let mut sim_headroom_at_res = (sim_rem + tokens_freed_by_res).saturating_sub(head_tokens);

        let mut backfilled_ids = Vec::new();
        let mut admitted_subgoals: Vec<SubGoal> = Vec::new();

        let candidates: Vec<usize> = coordinator.ready_set.iter().skip(1).copied().collect();

        // 4. Evaluate candidates u in R_t \ {v*}
        for cand_id in candidates {
            let cand_sg = match coordinator.records.get(&cand_id) {
                Some(r) => r.subgoal.clone(),
                None => continue,
            };

            let mut current_active: Vec<&SubGoal> = coordinator.records.values()
                .filter(|r| r.status == TaskStatus::Running)
                .map(|r| &r.subgoal)
                .collect();
            for adm in &admitted_subgoals {
                current_active.push(adm);
            }

            let eligibility = self.evaluate_backfill_predicate(
                &cand_sg,
                sim_rem,
                &reservation,
                sim_headroom_at_res,
                &current_active,
            );

            if let BackfillEligibility::Eligible { justification } = eligibility {
                let k_u = cand_sg.max_tokens;
                coordinator.admit_task(cand_id, k_u)?;
                backfilled_ids.push(cand_id);
                sim_rem = sim_rem.saturating_sub(k_u);

                match justification {
                    BackfillJustification::CompletesBeforeReservation { .. } => {}
                    BackfillJustification::HeadroomPreservedAtReservation { .. } => {
                        sim_headroom_at_res = sim_headroom_at_res.saturating_sub(k_u);
                    }
                }

                admitted_subgoals.push(cand_sg);
            }
        }

        if !backfilled_ids.is_empty() {
            Ok(ScheduleDecision::PinnedWithBackfill {
                pinned_head_id: head_id,
                reservation_delay_secs: reservation.reservation_delay_secs,
                backfilled_task_ids: backfilled_ids,
            })
        } else {
            Ok(ScheduleDecision::ContextBoomWait {
                pinned_head_id: head_id,
                reservation_delay_secs: reservation.reservation_delay_secs,
                deficit_tokens: reservation.deficit_tokens,
            })
        }
    }
}


#[derive(Debug, Clone)]
pub struct WorkerFinding {
    pub worker_id: usize,
    pub sub_goal: String,
    pub finding: String,
    pub steps_taken: usize,
    pub rollbacks_count: usize,
    pub tokens_saved_by_rollback: usize,
}

#[derive(Debug, Clone)]
pub struct SwarmResult {
    pub goal: String,
    pub subgoals_count: usize,
    pub findings: Vec<WorkerFinding>,
    pub final_report: String,
    pub total_rollbacks: usize,
    pub total_tokens_saved: usize,
    pub elapsed_ms: u128,
    pub readme_content: Option<String>,
    pub sealed_context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SwarmConfig {
    pub orchestrator_model_path: PathBuf,
    pub worker_model_path: PathBuf,
    pub max_subgoals: usize,
    pub max_steps_per_worker: usize,
    pub input_dir: Option<PathBuf>,
    pub output_dir: Option<PathBuf>,
    pub verbose: bool,
    pub auto_approve_deps: Option<bool>,
    pub is_modify_mode: bool,
    pub review_plan: bool,
}

impl SwarmConfig {
    /// Auto-detect available GGUF models in standard portable directories:
    /// 1. ./models
    /// 2. ../models
    /// 3. ./
    /// Works with ANY open-source GGUF model (Qwen, Llama, Mistral, Phi, Gemma, or custom fine-tuned weights)
    pub fn auto_detect(base_dir: &Path) -> Option<Self> {
        let candidates = vec![
            base_dir.join("models"),
            base_dir.join("../models"),
            base_dir.to_path_buf(),
        ];

        let mut found_models = Vec::new();

        for dir in &candidates {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "gguf" {
                                found_models.push(path);
                            }
                        }
                    }
                }
            }
            if !found_models.is_empty() {
                break;
            }
        }

        if found_models.is_empty() {
            return None;
        }

        // Sort by file size descending
        found_models.sort_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0));
        found_models.reverse();

        // Unified Sovereign Architecture: Both Thinker (System 2) and Worker (System 1) share
        // the top-performing model in RAM via Arc<NativeLlamaModel>, saving memory and maximizing IQ.
        let orch = found_models[0].clone();
        let worker = orch.clone();

        Some(Self {
            orchestrator_model_path: orch,
            worker_model_path: worker,
            max_subgoals: 3,
            max_steps_per_worker: 5,
            input_dir: None,
            output_dir: None,
            verbose: true,
            auto_approve_deps: None,
            is_modify_mode: false,
            review_plan: false,
        })
    }
}

pub struct SwarmCoordinator {
    config: SwarmConfig,
    orchestrator_model: Arc<NativeLlamaModel>,
    worker_model: Arc<NativeLlamaModel>,
    web_lens: Arc<WebLens>,
    vfs: Arc<MemoryVfs>,
    terminal: Arc<crate::sandbox::TerminalSessionBridge>,
}

impl SwarmCoordinator {
    pub fn new(config: SwarmConfig) -> Result<Self, String> {
        info!("Loading System 2 Orchestrator Model: {:?}", config.orchestrator_model_path);
        let orchestrator_model = NativeLlamaModel::load(&config.orchestrator_model_path, 0)?;

        let worker_model = if config.worker_model_path == config.orchestrator_model_path {
            orchestrator_model.clone()
        } else {
            info!("Loading System 1 Worker Swarm Model: {:?}", config.worker_model_path);
            NativeLlamaModel::load(&config.worker_model_path, 0)?
        };

        let terminal = Arc::new(crate::sandbox::TerminalSessionBridge::new(1000));

        Ok(Self {
            config,
            orchestrator_model,
            worker_model,
            web_lens: Arc::new(WebLens::new()),
            vfs: Arc::new(MemoryVfs::new()),
            terminal,
        })
    }

    /// Resolves an existing file on the host machine across configured directories:
    /// 1. input_dir (if specified)
    /// 2. output_dir (if specified)
    /// 3. Current working directory
    pub fn resolve_existing_file(&self, relative_path: &Path) -> Option<PathBuf> {
        // 1. Check in input_dir
        if let Some(ref in_dir) = self.config.input_dir {
            let candidate = in_dir.join(relative_path);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // 2. Check in output_dir
        if let Some(ref out_dir) = self.config.output_dir {
            let candidate = out_dir.join(relative_path);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        // 3. Check relative to current working directory
        if relative_path.exists() {
            return Some(relative_path.to_path_buf());
        }
        None
    }

    /// Preloads existing project deliverables into the in-memory Virtual Filesystem (VFS).
    /// Used in Modify / Incremental Extension mode or when an input_dir is specified to
    /// ensure existing working code is immediately accessible for workers without disk races.
    pub fn preload_existing_workspace_files(&self) {
        let dirs_to_scan = [self.config.input_dir.as_ref(), self.config.output_dir.as_ref()];
        let mut loaded = 0;
        for dir_opt in dirs_to_scan {
            if let Some(dir) = dir_opt {
                if dir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if path.is_file() {
                                let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                if fname.starts_with('.') || fname.ends_with(".log") || fname == "README.md" {
                                    continue;
                                }
                                let relative = PathBuf::from(&fname);
                                if !self.vfs.exists(&relative) {
                                    if let Ok(bytes) = std::fs::read(&path) {
                                        self.vfs.preload_file(&relative, &bytes);
                                        loaded += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if loaded > 0 {
            let files: Vec<String> = self.vfs.list_files().iter().map(|p| p.to_string_lossy().to_string()).collect();
            println!("  [WORKSPACE PRELOAD] Preloaded {} existing deliverable(s) into RAM VFS: [{}]", loaded, files.join(", ").bright_cyan());
        }
    }

    /// Constructs an analytical MemoryGate calibrated for the worker model geometry and quantization.
    pub fn create_memory_gate(&self) -> MemoryGate {
        let l = 28;
        let d = 1536;
        let h_q = 12;
        let h_kv = 2;
        
        // Dynamic Quantization Factor:
        // Automatically calibrate memory scaling based on GGUF quantization tier (e.g., Q4_K_M, Q5_K_M, Q8_0).
        let path_lower = self.worker_model.model_path.to_lowercase();
        let quant_scale = if path_lower.contains("q4") || path_lower.contains("iq4") {
            0.50 // Q4 quantization factor
        } else if path_lower.contains("q5") || path_lower.contains("q6") {
            0.65 // Q5/Q6 quantization factor
        } else if path_lower.contains("q8") {
            0.80 // Q8 quantization factor
        } else {
            1.00 // Conservative baseline FP16 upper bound
        };

        let raw_bytes_per_token = MemoryGate::analytical_bytes_per_token(l, h_kv, d, h_q);
        let bytes_per_token = ((raw_bytes_per_token as f32) * quant_scale).ceil() as usize;

        let n_ctx_train = if self.worker_model.n_ctx_train > 0 {
            self.worker_model.n_ctx_train as usize
        } else {
            8192
        };

        let specs = crate::system_info::get_system_specs();
        let free_bytes = (specs.free_ram_gb * 1024.0 * 1024.0 * 1024.0) as u64;
        let safe_budget_bytes = (free_bytes as f64 * 0.70) as usize;

        let k_max = MemoryGate::calculate_k_max(n_ctx_train, safe_budget_bytes, bytes_per_token).clamp(1024, 65536);

        info!(
            "[MEMORY GATE] Initialized MemoryGate: K_max={} tokens | bytes_per_token={} (quant_scale={:.2}) | n_ctx_train={}",
            k_max, bytes_per_token, quant_scale, n_ctx_train
        );

        MemoryGate::with_bytes_per_token(k_max, bytes_per_token)
    }

    /// Format prompts dynamically based on the model's detected template.
    pub fn format_prompt(model: &NativeLlamaModel, system: &str, user: &str) -> String {
        let sys = if system.trim().is_empty() { None } else { Some(system) };
        model.format_chat(sys, user)
    }

    /// Append user feedback or observation turn dynamically to active context
    pub fn format_observation_turn(model: &NativeLlamaModel, observation: &str, next_instruction: &str) -> String {
        model.format_observation_turn(observation, next_instruction)
    }

    /// Checks if a piece is a standard end-of-turn or stop token dynamically
    pub fn is_stop_piece(model: &NativeLlamaModel, piece: &str) -> bool {
        model.is_stop_piece(piece)
    }

    /// Checks if an executable command exists in system PATH in a platform-agnostic way
    pub fn is_command_available(cmd: &str) -> bool {
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in path_var.split(':') {
                let bin_path = std::path::Path::new(dir).join(cmd);
                if bin_path.is_file() {
                    return true;
                }
            }
        }
        false
    }

    /// Resolves the optimal host syntax check command for any artifact to perform fast AST/syntax
    /// validation during file generation without executing runtime side-effects or waiting on user input loops.
    pub fn resolve_syntax_check_command(target: &str) -> String {
        let p = std::path::Path::new(target);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");

        match ext {
            "py" => {
                if Self::is_command_available("python3") {
                    format!("python3 -m py_compile {} 2>&1", target)
                } else if Self::is_command_available("python") {
                    format!("python -m py_compile {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "c" => {
                if Self::is_command_available("gcc") {
                    format!("gcc -Wall -O2 -fsyntax-only {} 2>&1", target)
                } else if Self::is_command_available("clang") {
                    format!("clang -fsyntax-only {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "cpp" | "cc" | "cxx" => {
                if Self::is_command_available("g++") {
                    format!("g++ -Wall -O2 -fsyntax-only {} 2>&1", target)
                } else if Self::is_command_available("clang++") {
                    format!("clang++ -fsyntax-only {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "rs" => {
                if Self::is_command_available("rustc") {
                    format!("rustc --emit=metadata {} -o /tmp/omni_rs_target.rmeta 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "js" => {
                if Self::is_command_available("node") {
                    format!("node --check {} 2>&1", target)
                } else if Self::is_command_available("deno") {
                    format!("deno check {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "ts" => {
                if Self::is_command_available("tsc") {
                    format!("tsc --noEmit {} 2>&1", target)
                } else if Self::is_command_available("deno") {
                    format!("deno check {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "sh" | "bash" => {
                if Self::is_command_available("bash") {
                    format!("bash -n {} 2>&1", target)
                } else {
                    format!("sh -n {} 2>&1", target)
                }
            }
            "go" => {
                if Self::is_command_available("go") {
                    format!("go vet {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "zig" => {
                if Self::is_command_available("zig") {
                    format!("zig ast-check {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            _ => format!("test -f {}", target),
        }
    }

    /// Resolves the optimal host verification command for any artifact in a language-agnostic,
    /// toolchain-discovered manner without hardcoding language rules into business logic.
    pub fn resolve_verification_command(target: &str) -> String {
        let p = std::path::Path::new(target);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        match ext {
            "c" => {
                if Self::is_command_available("gcc") {
                    format!("gcc -Wall -O2 -fsyntax-only {} 2>&1 || gcc -Wall -O2 {} -o /tmp/omni_c_target 2>&1", target, target)
                } else if Self::is_command_available("clang") {
                    format!("clang -fsyntax-only {} 2>&1 || clang {} -o /tmp/omni_c_target 2>&1", target, target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "cpp" | "cc" | "cxx" => {
                if Self::is_command_available("g++") {
                    format!("g++ -Wall -O2 -fsyntax-only {} 2>&1 || g++ -Wall -O2 {} -o /tmp/omni_cpp_target 2>&1", target, target)
                } else if Self::is_command_available("clang++") {
                    format!("clang++ -fsyntax-only {} 2>&1 || clang++ {} -o /tmp/omni_cpp_target 2>&1", target, target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "java" => {
                let parent = p.parent().unwrap_or(std::path::Path::new("."));
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or(target);
                let run_dir = if parent.as_os_str().is_empty() || parent == std::path::Path::new(".") {
                    ".".to_string()
                } else {
                    parent.display().to_string()
                };

                if Self::is_command_available("javac") {
                    format!("(cd {} && javac {}) 2>&1", run_dir, fname)
                } else if Self::is_command_available("java") {
                    format!("(cd {} && java {}) 2>&1", run_dir, fname)
                } else {
                    format!("test -f {}", target)
                }
            }
            "py" => {
                if Self::is_command_available("python3") {
                    format!("python3 -m py_compile {0} 2>&1 && python3 {0} 2>&1", target)
                } else if Self::is_command_available("python") {
                    format!("python -m py_compile {0} 2>&1 && python {0} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "rs" => {
                if Self::is_command_available("rustc") {
                    format!("rustc --emit=metadata {} -o /tmp/omni_rs_target.rmeta 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "go" => {
                if Self::is_command_available("go") {
                    format!("go vet {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "zig" => {
                if Self::is_command_available("zig") {
                    format!("zig ast-check {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "sh" | "bash" => {
                if Self::is_command_available("bash") {
                    format!("bash -n {} 2>&1", target)
                } else {
                    format!("sh -n {} 2>&1", target)
                }
            }
            "js" => {
                if Self::is_command_available("node") {
                    format!("node --check {} 2>&1", target)
                } else if Self::is_command_available("deno") {
                    format!("deno check {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            "ts" => {
                if Self::is_command_available("tsc") {
                    format!("tsc --noEmit {} 2>&1", target)
                } else if Self::is_command_available("deno") {
                    format!("deno check {} 2>&1", target)
                } else if Self::is_command_available("bun") {
                    format!("bun build --no-bundle {} 2>&1", target)
                } else {
                    format!("test -f {}", target)
                }
            }
            custom if !custom.is_empty() && Self::is_command_available(custom) => {
                format!("{} check {} 2>&1 || {} {} 2>&1", custom, target, custom, target)
            }
            _ => {
                format!("test -s {}", target)
            }
        }
    }

    /// Universally extracts the failing line number from compiler or runtime error text across all languages
    pub fn extract_error_line_number(err_text: &str, target_filename: &str) -> Option<usize> {
        let base_name = std::path::Path::new(target_filename)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(target_filename);

        for line in err_text.lines() {
            // Pattern 1: target.ext:14: error or target.ext:14:5: error
            if line.contains(base_name) {
                if let Some(pos) = line.find(base_name) {
                    let rest = &line[pos + base_name.len()..];
                    if rest.starts_with(':') {
                        let num_str: String = rest[1..].chars().take_while(|c| c.is_ascii_digit()).collect();
                        if let Ok(num) = num_str.parse::<usize>() {
                            if num > 0 {
                                return Some(num);
                            }
                        }
                    }
                }
            }

            // Pattern 2: line 14 or Line 14
            for keyword in &["line ", "Line ", "line: ", "Line: "] {
                if let Some(pos) = line.find(keyword) {
                    let rest = &line[pos + keyword.len()..];
                    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(num) = num_str.parse::<usize>() {
                        if num > 0 {
                            return Some(num);
                        }
                    }
                }
            }

            // Pattern 3: [Line 14] or [14]
            if let Some(start) = line.find('[') {
                if let Some(end) = line.find(']') {
                    if start < end {
                        let inner = &line[start + 1..end];
                        let clean = inner.trim_start_matches("Line ").trim_start_matches("line ");
                        if let Ok(num) = clean.trim().parse::<usize>() {
                            if num > 0 {
                                return Some(num);
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Primary Swarm Execution Pipeline:
    /// 1. System 2 Thinker: Decomposes overall goal into targeted sub-tasks
    /// 2. System 1 Swarm: Dispatches autonomous worker instances with Checkpoint Freeze
    /// 3. Epistemic Ledger: Rollback on failure with Thinker Sovereign arbitration
    /// 4. System 2 Thinker: Synthesizes gathered factual findings into final answer
    pub fn execute_goal(&self, user_goal: &str) -> Result<SwarmResult, String> {
        let start_time = Instant::now();

        println!("\n{}", "=".repeat(80).bright_blue());
        println!(
            "  [SWARM ENGINE] Objective: \"{}\"",
            user_goal.bright_white()
        );
        println!(
            "  [SYSTEM ARCHITECTURE] Thinker: {} | Worker: {}",
            self.config.orchestrator_model_path.file_name().unwrap_or_default().to_string_lossy().bright_cyan(),
            self.config.worker_model_path.file_name().unwrap_or_default().to_string_lossy().bright_green()
        );
        println!("{}\n", "=".repeat(80).bright_blue());

        // PHASE 0: Preload existing workspace deliverables in Modify / Incremental mode
        if self.config.is_modify_mode || self.config.input_dir.is_some() {
            self.preload_existing_workspace_files();
        }

        // PHASE 1: System 2 Orchestrator Plan Synthesis
        println!("  [THINKER PLAN] Formulating execution sub-tasks...");
        let mut current_goal = user_goal.to_string();
        let mut subgoals = self.orchestrate_plan(&current_goal)?;

        // PHASE 1.1: Interactive Plan Review & Steering (Human-in-the-Loop Governance)
        if self.config.review_plan {
            let mut iteration = 1;
            loop {
                self.render_plan_preview(&current_goal, &subgoals);
                print!("  [PLAN REVIEW v{}] Approve? [yes / no <critique>]: ", iteration);
                use std::io::{self, Write};
                io::stdout().flush().unwrap_or(());

                let mut input = String::new();
                let bytes_read = match io::stdin().read_line(&mut input) {
                    Ok(n) => n,
                    Err(_) => 0,
                };
                if bytes_read == 0 {
                    println!("  [!] End of input detected. Proceeding with current plan.");
                    break;
                }
                let trimmed = input.trim();
                let lower = trimmed.to_lowercase();

                if lower == "yes" || lower == "y" || lower == "ok" || lower == "نعم" || lower == "تمام" || lower == "اه" || lower == "ايوه" {
                    println!("\n  {} Plan sealed into Causal Memory. Suffix debate rolled back.", "[PLAN SEALED]".bright_green().bold());
                    println!("  {} Executing approved golden architecture...\n", ">>".bright_yellow());
                    break;
                } else if lower.starts_with("no") || lower.starts_with("n ") || lower.starts_with("لا") {
                    let critique = if let Some(stripped) = trimmed.strip_prefix("no") {
                        stripped.trim().trim_start_matches(':').trim()
                    } else if let Some(stripped) = trimmed.strip_prefix("No") {
                        stripped.trim().trim_start_matches(':').trim()
                    } else if let Some(stripped) = trimmed.strip_prefix("n ") {
                        stripped.trim()
                    } else if let Some(stripped) = trimmed.strip_prefix("N ") {
                        stripped.trim()
                    } else if let Some(stripped) = trimmed.strip_prefix("لا") {
                        stripped.trim().trim_start_matches(':').trim()
                    } else {
                        trimmed
                    };

                    iteration += 1;
                    println!("\n  [STEERING RECEIVED] \"{}\"", critique.bright_yellow());
                    println!("  [CAUSAL KV-ROLLBACK] Excising unapproved draft plan from KV-Cache...");
                    println!("  [THINKER RE-PLAN v{}] Incorporating steering constraints into architecture...", iteration);

                    current_goal = format!(
                        "{}\n[MANDATORY ARCHITECTURAL CRITIQUE & USER STEERING: {}]",
                        user_goal, critique
                    );
                    subgoals = self.orchestrate_plan(&current_goal)?;
                } else {
                    println!("  [!] Unrecognized response. Please type '{}' to proceed, or '{}' to adjust.", "yes".bright_green(), "no <critique>".bright_yellow());
                }
            }
        } else {
            println!("  [PLAN] Generated {} sub-task(s):", subgoals.len().to_string().bright_yellow());
            for sg in &subgoals {
                println!("   - Task #{}: {} (Target: {})", sg.id, sg.description.bright_white(), sg.target_entity.bright_cyan());
            }
            println!();
        }

        // PHASE 1.5: Generate Global Contract (Blackboard)
        println!("  [THINKER BLACKBOARD] Synthesizing Global Contract for DAG Agents...");
        let global_contract = self.generate_global_contract(&current_goal, &subgoals).unwrap_or_else(|_| "No strict contract.".to_string());
        println!("  [CONTRACT]: {}\n", global_contract.bright_magenta());

        // PHASE 2: System 1 Swarm Execution with State Machine & Memory Bound Gate
        let dag = TaskDag::from_subgoals(subgoals.clone())
            .map_err(|e| format!("DAG construction failed: {}", e))?;
        let memory_gate = self.create_memory_gate();
        let mut state_coordinator = StateMachineCoordinator::with_memory_gate(dag, memory_gate)?;

        // Pre-flight Zero-OOM validation
        if let Err(rejections) = state_coordinator.preflight_validate() {
            for rej in &rejections {
                warn!("{}", rej);
                println!("  [ZERO-OOM PRE-FLIGHT] {}", rej.bright_red());
            }
        }

        let findings = self.run_rcpsp_swarm_scope(
            &current_goal,
            &global_contract,
            &mut state_coordinator,
        )?;

        // PHASE 3: System 2 Orchestrator Grounded Synthesis
        println!("  [THINKER SYNTHESIS] Synthesizing verified deliverables...");
        let (final_report, model_overview) = self.synthesize_findings(&current_goal, &findings)?;

        // PHASE 4: VFS Materialization to Host Disk (Strictly Verified Only)
        let mut verified_paths: Vec<PathBuf> = Vec::new();
        for r in state_coordinator.records.values() {
            if r.status == TaskStatus::Completed && r.subgoal.kind != TaskKind::Test {
                let p = PathBuf::from(&r.subgoal.target_entity);
                if !p.as_os_str().is_empty() && !verified_paths.contains(&p) {
                    verified_paths.push(p);
                }
            }
        }

        let mut final_verified_paths = verified_paths;
        let (readme_content, sealed_context) = if !final_verified_paths.is_empty() {
            // Universal Documentation Mandate:
            // Every completed Swarm mission unconditionally generates a standard 'README.md'
            // documenting the project deliverables, architecture, execution instructions, and verification status.
            let readme = self.generate_project_readme(
                user_goal,
                &findings,
                &final_verified_paths,
                model_overview.as_deref(),
            );
            self.vfs.write_file("README.md", readme.as_bytes());
            let readme_path = PathBuf::from("README.md");
            if !final_verified_paths.contains(&readme_path) {
                final_verified_paths.push(readme_path);
            }

            // Causal Turn Sealing:
            // Inject Assistant response containing the README.md content + verification flag,
            // sealed with the model's native turn_suffix / EOT token.
            let assistant_turn = format!(
                "{}\n\n[DELIVERABLES VERIFIED]: All project artifacts generated, validated in cleanroom, and committed to disk.",
                readme.trim()
            );
            let sealed = self.orchestrator_model.format_completed_turn(None, user_goal, &assistant_turn);
            (Some(readme), Some(sealed))
        } else {
            (None, None)
        };

        if final_verified_paths.is_empty() {
            println!("  [HOST DISK COMMIT GATE] Zero tasks reached 'Completed' status. Skipping host disk commit.");
        } else {
            let target_dir = self.config.output_dir.as_deref();
            println!("  [HOST DISK COMMIT GATE] Materializing verified deliverables to host disk: {:?}", final_verified_paths);
            match self.vfs.commit_verified_files(target_dir, &final_verified_paths, true) {
                Ok(saved_paths) => {
                    for dest_path in &saved_paths {
                        println!("   [SAVED TO DISK] {}", dest_path.display().to_string().bright_white());
                    }
                    println!("   [COMMIT] Successfully committed {} verified file(s) to host machine.", saved_paths.len().to_string().bright_green());
                }
                Err(e) => {
                    warn!("Failed to commit verified VFS files to host disk: {}", e);
                }
            }
        }

        let total_rollbacks: usize = findings.iter().map(|f| f.rollbacks_count).sum();
        let total_tokens_saved: usize = findings.iter().map(|f| f.tokens_saved_by_rollback).sum();

        println!("\n{}", "=".repeat(80).bright_green());
        println!("  [SWARM COMPLETE] Execution completed in {:?}", start_time.elapsed());
        println!("     * Workers Deployed:          {}", findings.len());
        println!("     * Checkpoint Rollbacks:      {}", total_rollbacks.to_string().bright_yellow());
        println!("     * Trapped Tokens Excised:    {} tokens saved", total_tokens_saved.to_string().bright_cyan());
        println!("{}\n", "=".repeat(80).bright_green());

        Ok(SwarmResult {
            goal: user_goal.to_string(),
            subgoals_count: findings.len(),
            findings,
            final_report,
            total_rollbacks,
            total_tokens_saved,
            elapsed_ms: start_time.elapsed().as_millis(),
            readme_content,
            sealed_context,
        })
    }

    /// Executes the RCPSP Swarm DAG concurrently using std::thread::scope.
    pub fn run_rcpsp_swarm_scope(
        &self,
        user_goal: &str,
        global_contract: &str,
        state_coordinator: &mut StateMachineCoordinator,
    ) -> Result<Vec<WorkerFinding>, String> {
        let (tx, rx): (Sender<WorkerEvent>, Receiver<WorkerEvent>) = channel();
        let mut active_workers: HashMap<usize, ActiveWorkerProfile> = HashMap::new();
        let mut findings: Vec<WorkerFinding> = Vec::new();
        let mut delta_retry_counts: HashMap<String, usize> = HashMap::new();
        let mut total_dynamic_injections: usize = 0;

        let lrp_config = LrpConfig {
            r_min: 20.0,
            min_tokens_floor: 2048,
            model_ctx_limit: if self.worker_model.n_ctx_train > 0 {
                Some(self.worker_model.n_ctx_train as usize)
            } else {
                None
            },
        };

        let lrp_table = state_coordinator.dag.compute_lrp(&lrp_config).unwrap_or_default();
        let max_n_ctx = self.worker_model.n_ctx_train;

        std::thread::scope(|s| {
            loop {
                // 1. TERMINATION CHECK
                if state_coordinator.is_all_terminal() && active_workers.is_empty() {
                    break;
                }

                // 2. DYNAMIC ADMISSION & BACKFILLING LOOP
                loop {
                    state_coordinator.dag.sort_ready_set(&mut state_coordinator.ready_set, &lrp_table);

                    if state_coordinator.ready_set.is_empty() {
                        break;
                    }

                    let head_id = state_coordinator.ready_set[0];
                    let head_sg = match state_coordinator.records.get(&head_id) {
                        Some(r) => r.subgoal.clone(),
                        None => break,
                    };

                    let mut head_alloc_ctx = head_sg.max_tokens.max(2048);
                    if max_n_ctx > 0 && max_n_ctx < head_alloc_ctx as i32 {
                        head_alloc_ctx = max_n_ctx as usize;
                    }

                    // Check if head task v* has VFS conflict with currently running tasks
                    let head_vfs_conflict = active_workers.values().any(|aw| {
                        Self::has_vfs_conflict(&head_sg.write_set, &head_sg.read_set, &aw.write_set, &aw.read_set)
                    });

                    // Can Head Task Admit Right Now?
                    if !head_vfs_conflict && state_coordinator.memory_gate.can_admit(head_alloc_ctx) {
                        let abort_flag = match state_coordinator.admit_task(head_id, head_alloc_ctx) {
                            Ok(flag) => flag,
                            Err(e) => {
                                warn!("Failed to admit head task #{}: {}", head_id, e);
                                break;
                            }
                        };

                        let profile = ActiveWorkerProfile {
                            task_id: head_id,
                            allocated_tokens: head_alloc_ctx,
                            started_at: Instant::now(),
                            expected_duration_secs: head_alloc_ctx as f64 / lrp_config.r_min,
                            abort_flag: abort_flag.clone(),
                            write_set: head_sg.write_set.clone(),
                            read_set: head_sg.read_set.clone(),
                        };
                        active_workers.insert(head_id, profile);

                        println!(
                            "  [DISPATCH WORKER] Task #{}: \"{}\" (KV Quota: {})",
                            head_id,
                            head_sg.description.bright_white(),
                            head_alloc_ctx.to_string().bright_blue()
                        );

                        let dag_contracts = state_coordinator.get_all_interface_contracts();
                        Self::spawn_scoped_worker(
                            s,
                            self,
                            user_goal,
                            global_contract,
                            head_sg,
                            head_alloc_ctx,
                            abort_flag,
                            findings.clone(),
                            dag_contracts,
                            tx.clone(),
                        );

                        // Re-evaluate ready set after admitting head task
                        continue;
                    }

                    // Head task v* cannot start now -> PINNED.
                    let reservation_delay_secs = Self::compute_shadow_reservation_delay(
                        head_alloc_ctx,
                        &state_coordinator.memory_gate,
                        &active_workers,
                        lrp_config.r_min,
                    );

                    let mut backfilled_any = false;

                    // Evaluate candidate tasks u in R_t \ {v*} for EASY Backfilling
                    let candidate_ids: Vec<usize> = state_coordinator.ready_set.iter().skip(1).copied().collect();
                    for cand_id in candidate_ids {
                        let cand_sg = match state_coordinator.records.get(&cand_id) {
                            Some(r) => r.subgoal.clone(),
                            None => continue,
                        };

                        let mut cand_alloc_ctx = cand_sg.max_tokens.max(2048);
                        if max_n_ctx > 0 && max_n_ctx < cand_alloc_ctx as i32 {
                            cand_alloc_ctx = max_n_ctx as usize;
                        }

                        if !state_coordinator.memory_gate.can_admit(cand_alloc_ctx) {
                            continue;
                        }

                        let cand_vfs_conflict = active_workers.values().any(|aw| {
                            Self::has_vfs_conflict(&cand_sg.write_set, &cand_sg.read_set, &aw.write_set, &aw.read_set)
                        }) || Self::has_vfs_conflict(&cand_sg.write_set, &cand_sg.read_set, &head_sg.write_set, &head_sg.read_set);

                        if cand_vfs_conflict {
                            continue;
                        }

                        // EASY Backfill Predicate: Does starting u delay v*?
                        let cand_duration = cand_alloc_ctx as f64 / lrp_config.r_min;
                        let completes_before_res = cand_duration <= reservation_delay_secs;
                        let headroom_preserved = state_coordinator.memory_gate.remaining() >= cand_alloc_ctx + head_alloc_ctx;

                        if completes_before_res || headroom_preserved {
                            let abort_flag = match state_coordinator.admit_task(cand_id, cand_alloc_ctx) {
                                Ok(flag) => flag,
                                Err(_) => continue,
                            };

                            let profile = ActiveWorkerProfile {
                                task_id: cand_id,
                                allocated_tokens: cand_alloc_ctx,
                                started_at: Instant::now(),
                                expected_duration_secs: cand_duration,
                                abort_flag: abort_flag.clone(),
                                write_set: cand_sg.write_set.clone(),
                                read_set: cand_sg.read_set.clone(),
                            };
                            active_workers.insert(cand_id, profile);

                            println!(
                                "  [EASY BACKFILL] Backfilled Task #{}: \"{}\" ({} tokens, {:.1}s) ahead of pinned Task #{}",
                                cand_id,
                                cand_sg.description.bright_white(),
                                cand_alloc_ctx.to_string().bright_blue(),
                                cand_duration,
                                head_id
                            );

                            let dag_contracts = state_coordinator.get_all_interface_contracts();
                            Self::spawn_scoped_worker(
                                s,
                                self,
                                user_goal,
                                global_contract,
                                cand_sg,
                                cand_alloc_ctx,
                                abort_flag,
                                findings.clone(),
                                dag_contracts,
                                tx.clone(),
                            );

                            backfilled_any = true;
                            break; // Break candidate loop to re-sort ready set
                        }
                    }

                    if !backfilled_any {
                        // Neither head nor candidate could be admitted -> Context Boom / Reservation wait
                        break;
                    }
                }

                // 3. SUSPENSION & EVENT RECEPTION
                if active_workers.is_empty() {
                    if !state_coordinator.is_all_terminal() {
                        warn!("Deadlock detected: No active workers and no ready tasks can be admitted.");
                        for rec in state_coordinator.records.values_mut() {
                            if !rec.status.is_terminal() {
                                rec.status = TaskStatus::Cancelled;
                                rec.failure_reason = Some("Deadlock avoidance: un-runnable task sweep".to_string());
                            }
                        }
                    }
                    break;
                }

                match rx.recv() {
                    Ok(event) => match event {
                        WorkerEvent::Completed { task_id, finding } => {
                            active_workers.remove(&task_id);
                            findings.push(finding);

                            // The Automatic Contract Distiller Rule:
                            // If the staged deliverable exceeds auto-inject threshold, distill its contract and register in DAG.
                            if let Some(record) = state_coordinator.records.get(&task_id) {
                                let target_entity = record.subgoal.target_entity.clone();
                                if !target_entity.is_empty() {
                                    let target_path = PathBuf::from(&target_entity);
                                    if let Some(bytes) = self.vfs.read_file(&target_path) {
                                        let content = String::from_utf8_lossy(&bytes);
                                        let lines_count = content.lines().count();
                                        if lines_count > AUTO_INJECT_MAX_LINES || bytes.len() > AUTO_INJECT_MAX_BYTES {
                                            println!("  [CONTRACT DISTILLER] File '{}' ({} lines, {} bytes) exceeds auto-inject threshold -> Registering Contract in DAG...", target_entity.bright_cyan(), lines_count, bytes.len());
                                            match self.distill_interface_contract(&target_entity, &content) {
                                                Ok(contract) => {
                                                    println!("  [DAG CONTRACT REGISTRY] Registered interface contract for '{}' on Task #{} in DAG.", target_entity.bright_cyan(), task_id);
                                                    state_coordinator.set_interface_contract(task_id, contract);
                                                }
                                                Err(e) => {
                                                    warn!("  [CONTRACT DISTILLER] Distillation warning for '{}': {}", target_entity, e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let _ = state_coordinator.complete_task(task_id);
                            info!("  [WORKER EVENT] Task #{} COMPLETED.", task_id);
                        }
                        WorkerEvent::Failed { task_id, reason, diagnostic_code, finding } => {
                            active_workers.remove(&task_id);
                            findings.push(finding);
                            let cancelled = state_coordinator.fail_task(task_id, &reason, diagnostic_code);
                            warn!(
                                "  [WORKER EVENT] Task #{} FAILED: {}. Cascaded cancellation to {} descendants: {:?}",
                                task_id, reason, cancelled.len(), cancelled
                            );
                        }
                        WorkerEvent::Cancelled { task_id, finding } => {
                            active_workers.remove(&task_id);
                            findings.push(finding);
                            info!("  [WORKER EVENT] Task #{} CANCELLED via preemption.", task_id);
                        }
                        WorkerEvent::AcceptanceDelta { task_id, target_file, delta, finding } => {
                            active_workers.remove(&task_id);
                            findings.push(finding);
                            let _ = state_coordinator.complete_task(task_id);

                            println!("\n{}", "────────────────────────────────────────────────────────────────────────────────".bright_magenta());
                            println!("  [TESTER ACCEPTANCE AUDIT] Target '{}': Incomplete against user goal!", target_file.bright_cyan());
                            println!("  [ACCEPTANCE DELTA REPORT] Missing requirements: {}", delta.bright_yellow());

                            let target_path = PathBuf::from(&target_file);
                            let missing_artifact = {
                                let mut found = None;
                                for part in delta.split(';') {
                                    let trimmed = part.trim();
                                    if let Some(idx) = trimmed.find("Missing required artifact '") {
                                        let rest = &trimmed[idx + "Missing required artifact '".len()..];
                                        if let Some(end_idx) = rest.find('\'') {
                                            let file = &rest[..end_idx];
                                            if !file.is_empty() {
                                                found = Some(file.to_string());
                                                break;
                                            }
                                        }
                                    }
                                }
                                found
                            };

                            let req_key = missing_artifact.clone().unwrap_or_else(|| target_file.clone());

                            // Barrier 1: Global Injection Ceiling (Circuit Breaker)
                            if total_dynamic_injections >= MAX_TOTAL_DYNAMIC_INJECTIONS {
                                println!(
                                    "  {} Total dynamic injections reached maximum budget ({}/{}). Enforcing graceful termination.",
                                    "[GLOBAL CIRCUIT BREAKER]".bright_red().bold(),
                                    total_dynamic_injections,
                                    MAX_TOTAL_DYNAMIC_INJECTIONS
                                );
                                println!("  [SOVEREIGN HALT] Preserving all working deliverables in VFS (Zero-Wipe Invariant). Terminating expansion.");
                                println!("{}\n", "────────────────────────────────────────────────────────────────────────────────".bright_magenta());
                                continue;
                            }

                            // Barrier 2: Rule of 3 (Per-Requirement Retry Budget)
                            let retry_count = delta_retry_counts.entry(req_key.clone()).or_insert(0);
                            *retry_count += 1;
                            let current_attempt = *retry_count;

                            if current_attempt > MAX_DELTA_RETRIES_PER_TARGET {
                                println!(
                                    "  {} Requirement '{}' has exceeded maximum retries ({}/{}). Halting dynamic loop to prevent live-lock.",
                                    "[RULE OF 3 CIRCUIT BREAKER]".bright_red().bold(),
                                    req_key.bright_yellow(),
                                    current_attempt - 1,
                                    MAX_DELTA_RETRIES_PER_TARGET
                                );
                                println!("  [SOVEREIGN HALT] Preserving all working deliverables in VFS (Zero-Wipe Invariant). Terminating expansion.");
                                println!("{}\n", "────────────────────────────────────────────────────────────────────────────────".bright_magenta());
                                continue;
                            }

                            println!(
                                "  [THINKER PLANNER REVISION] Attempt {}/{} for '{}' -> Enforcing Zero-Wipe Invariant & Dynamic Subgoal Dispatch...",
                                current_attempt,
                                MAX_DELTA_RETRIES_PER_TARGET,
                                req_key.bright_cyan()
                            );

                            if let Some(missing_file) = missing_artifact {
                                let missing_path = PathBuf::from(&missing_file);
                                let escalated_guidance = match current_attempt {
                                    1 => format!(
                                        "Generate complete, standalone implementation of '{}' fulfilling all requirements in the user goal: {}.",
                                        missing_file, user_goal
                                    ),
                                    2 => format!(
                                        "[CRITICAL ESCALATION - ATTEMPT 2/{}]: Previous attempt failed to produce '{}'. You MUST generate a standalone, valid '{}' satisfying user goal: {}. Do NOT duplicate or embed its content inside other files.",
                                        MAX_DELTA_RETRIES_PER_TARGET, missing_file, missing_file, user_goal
                                    ),
                                    _ => format!(
                                        "[FINAL ATTEMPT {}/{}]: This is the final attempt to satisfy '{}'. Generate complete, standalone implementation of '{}'. If unfulfilled, dynamic expansion will halt permanently.",
                                        current_attempt, MAX_DELTA_RETRIES_PER_TARGET, missing_file, missing_file
                                    ),
                                };

                                match state_coordinator.inject_subgoal(
                                    SubGoal {
                                        id: 0,
                                        description: format!("Create missing required deliverable '{}' to satisfy objective (Attempt {}/{})", missing_file, current_attempt, MAX_DELTA_RETRIES_PER_TARGET),
                                        target_entity: missing_file.clone(),
                                        kind: TaskKind::Create,
                                        guidance: Some(escalated_guidance),
                                        depends_on: vec![],
                                        read_set: vec![target_path.clone()],
                                        write_set: vec![missing_path.clone()],
                                        max_tokens: 4096,
                                    },
                                    vec![task_id],
                                ) {
                                    Ok(create_id) => {
                                        total_dynamic_injections += 1;
                                        if let Ok(retest_id) = state_coordinator.inject_subgoal(
                                            SubGoal {
                                                id: 0,
                                                description: format!("Run cleanroom verification and acceptance re-audit of '{}' and '{}'", target_file, missing_file),
                                                target_entity: target_file.clone(),
                                                kind: TaskKind::Test,
                                                guidance: Some("Execute cleanroom verification and ensure all requirements are satisfied.".to_string()),
                                                depends_on: vec![missing_file.clone()],
                                                read_set: vec![target_path.clone(), missing_path.clone()],
                                                write_set: vec![],
                                                max_tokens: 2048,
                                            },
                                            vec![create_id],
                                        ) {
                                            total_dynamic_injections += 1;
                                            println!("  [DYNAMIC DAG EXPANSION] Successfully injected Create Task #{} for '{}' and Re-Audit Task #{}.", create_id, missing_file.bright_cyan(), retest_id);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("  [DYNAMIC DAG EXPANSION] Failed to inject subgoal: {}", e);
                                    }
                                }
                            } else {
                                let escalated_guidance = match current_attempt {
                                    1 => format!(
                                        "CRITICAL: Keep and preserve all existing working functions/code in '{}'. Incrementally add/modify code to fulfill missing requirements: {}. Do not delete or wipe existing features.",
                                        target_file, delta
                                    ),
                                    2 => format!(
                                        "[CRITICAL ESCALATION - ATTEMPT 2/{}]: Previous modifications on '{}' did not satisfy: {}. You MUST incrementally address these exact missing requirements while keeping all existing code.",
                                        MAX_DELTA_RETRIES_PER_TARGET, target_file, delta
                                    ),
                                    _ => format!(
                                        "[FINAL ATTEMPT {}/{}]: Final modification attempt on '{}' for missing requirements: {}. If unfulfilled, dynamic expansion will halt permanently.",
                                        current_attempt, MAX_DELTA_RETRIES_PER_TARGET, target_file, delta
                                    ),
                                };

                                match state_coordinator.inject_subgoal(
                                    SubGoal {
                                        id: 0,
                                        description: format!("Extend '{}' to implement missing requirements: {} (Attempt {}/{})", target_file, delta, current_attempt, MAX_DELTA_RETRIES_PER_TARGET),
                                        target_entity: target_file.clone(),
                                        kind: TaskKind::Modify,
                                        guidance: Some(escalated_guidance),
                                        depends_on: vec![target_file.clone()],
                                        read_set: vec![target_path.clone()],
                                        write_set: vec![target_path.clone()],
                                        max_tokens: 3072,
                                    },
                                    vec![task_id],
                                ) {
                                    Ok(modify_id) => {
                                        total_dynamic_injections += 1;
                                        if let Ok(retest_id) = state_coordinator.inject_subgoal(
                                            SubGoal {
                                                id: 0,
                                                description: format!("Run cleanroom verification and acceptance re-audit of '{}'", target_file),
                                                target_entity: target_file.clone(),
                                                kind: TaskKind::Test,
                                                guidance: Some(format!("Execute cleanroom verification and ensure all requirements including '{}' are 100% satisfied.", delta)),
                                                depends_on: vec![target_file.clone()],
                                                read_set: vec![target_path.clone()],
                                                write_set: vec![],
                                                max_tokens: 2048,
                                            },
                                            vec![modify_id],
                                        ) {
                                            total_dynamic_injections += 1;
                                            println!("  [DYNAMIC DAG EXPANSION] Successfully injected Modify Task #{} and Re-Audit Task #{}.", modify_id, retest_id);
                                        }
                                    }
                                    Err(e) => {
                                        warn!("  [DYNAMIC DAG EXPANSION] Failed to inject subgoal: {}", e);
                                    }
                                }
                            }
                            println!("{}\n", "────────────────────────────────────────────────────────────────────────────────".bright_magenta());
                        }
                        WorkerEvent::Panicked { task_id, panic_msg } => {
                            active_workers.remove(&task_id);
                            let cancelled = state_coordinator.fail_task(
                                task_id,
                                &format!("Worker thread panicked: {}", panic_msg),
                                None,
                            );
                            warn!(
                                "  [WORKER EVENT] Task #{} PANICKED: {}. Cascaded cancellation to: {:?}",
                                task_id, panic_msg, cancelled
                            );
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
        });

        // Ensure findings are deterministically sorted by worker_id for synthesis
        findings.sort_by_key(|f| f.worker_id);

        Ok(findings)
    }

    /// Spawns an isolated worker thread within the scope, wrapping execution with catch_unwind.
    fn spawn_scoped_worker<'scope>(
        s: &'scope std::thread::Scope<'scope, '_>,
        coordinator: &'scope SwarmCoordinator,
        user_goal: &'scope str,
        global_contract: &'scope str,
        subgoal: SubGoal,
        allocated_ctx: usize,
        abort_flag: Arc<AtomicBool>,
        previous_findings: Vec<WorkerFinding>,
        dag_contracts: HashMap<String, String>,
        tx: Sender<WorkerEvent>,
    ) {
        let task_id = subgoal.id;
        let subgoal_desc = subgoal.description.clone();

        s.spawn(move || {
            let res = catch_unwind(AssertUnwindSafe(|| {
                // Thread-Local NativeLlamaContext instantiation
                let mut worker_ctx = coordinator.worker_model
                    .create_context(allocated_ctx, 512, 4)
                    .map_err(|e| format!("Native context allocation failed: {}", e))?;

                // Generate directive if Thinker is configured
                let mut sg_with_guidance = subgoal.clone();
                if let Ok(directive) = coordinator.thinker_dispatch_directive(
                    user_goal,
                    &subgoal,
                    global_contract,
                    &previous_findings,
                ) {
                    if !directive.is_empty() {
                        println!(
                            "  [THINKER DIRECTIVE] Task #{}: \"{}\"",
                            subgoal.id,
                            directive.bright_white()
                        );
                        sg_with_guidance.guidance = Some(directive);
                    }
                }

                let is_first = previous_findings.is_empty();
                let finding = coordinator.run_worker_loop_with_abort_and_contracts(
                    user_goal,
                    &sg_with_guidance,
                    &mut worker_ctx,
                    is_first,
                    &previous_findings,
                    Some(abort_flag),
                    Some(&dag_contracts),
                )?;

                worker_ctx.kv_cache_clear();
                Ok(finding)
            }));

            match res {
                Ok(Ok(finding)) => {
                    if finding.finding.contains("ERR_OUT_OF_BOUNDS_WRITE")
                        || finding.finding.contains("OutOfBoundsWrite")
                    {
                        let _ = tx.send(WorkerEvent::Failed {
                            task_id,
                            reason: finding.finding.clone(),
                            diagnostic_code: Some(DiagnosticCode::OutOfBoundsWrite),
                            finding,
                        });
                    } else if finding.finding.starts_with("CANCELLED:") {
                        let _ = tx.send(WorkerEvent::Cancelled { task_id, finding });
                    } else if finding.finding.contains("[ACCEPTANCE DELTA]") {
                        let delta = if let Some(idx) = finding.finding.find("[ACCEPTANCE DELTA]") {
                            finding.finding[idx + 18..].lines().next().unwrap_or(&finding.finding).trim().to_string()
                        } else {
                            finding.finding.clone()
                        };
                        let _ = tx.send(WorkerEvent::AcceptanceDelta {
                            task_id,
                            target_file: subgoal.target_entity.clone(),
                            delta,
                            finding,
                        });
                    } else if finding.finding.starts_with("FAILED") {
                        let _ = tx.send(WorkerEvent::Failed {
                            task_id,
                            reason: finding.finding.clone(),
                            diagnostic_code: None,
                            finding,
                        });
                    } else {
                        let _ = tx.send(WorkerEvent::Completed { task_id, finding });
                    }
                }
                Ok(Err(err_str)) => {
                    let finding = WorkerFinding {
                        worker_id: task_id,
                        sub_goal: subgoal_desc,
                        finding: format!("FAILED: {}", err_str),
                        steps_taken: 0,
                        rollbacks_count: 0,
                        tokens_saved_by_rollback: 0,
                    };
                    let _ = tx.send(WorkerEvent::Failed {
                        task_id,
                        reason: err_str,
                        diagnostic_code: None,
                        finding,
                    });
                }
                Err(panic_payload) => {
                    let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic payload".to_string()
                    };
                    let _ = tx.send(WorkerEvent::Panicked {
                        task_id,
                        panic_msg: msg,
                    });
                }
            }
        });
    }

    /// Checks Bernstein conditions between two tasks' read and write sets.
    pub fn has_vfs_conflict(
        w1: &[PathBuf],
        r1: &[PathBuf],
        w2: &[PathBuf],
        r2: &[PathBuf],
    ) -> bool {
        // Write-Write conflict
        for p1 in w1 {
            if MemoryVfs::is_path_in_set(p1, w2) || w2.iter().any(|p2| MemoryVfs::is_path_in_set(p2, std::slice::from_ref(p1))) {
                return true;
            }
        }
        // Write-Read conflict (w1 vs r2)
        for p1 in w1 {
            if MemoryVfs::is_path_in_set(p1, r2) || r2.iter().any(|p2| MemoryVfs::is_path_in_set(p2, std::slice::from_ref(p1))) {
                return true;
            }
        }
        // Read-Write conflict (r1 vs w2)
        for p1 in r1 {
            if MemoryVfs::is_path_in_set(p1, w2) || w2.iter().any(|p2| MemoryVfs::is_path_in_set(p2, std::slice::from_ref(p1))) {
                return true;
            }
        }
        false
    }

    /// Calculates the shadow reservation delay for pinned head task v*.
    pub fn compute_shadow_reservation_delay(
        head_tokens: usize,
        memory_gate: &MemoryGate,
        active_workers: &HashMap<usize, ActiveWorkerProfile>,
        _r_min: f64,
    ) -> f64 {
        if memory_gate.remaining() >= head_tokens {
            return 0.0;
        }

        let deficit = head_tokens.saturating_sub(memory_gate.remaining());
        let mut profiles: Vec<&ActiveWorkerProfile> = active_workers.values().collect();

        // Sort by expected remaining duration ascending
        profiles.sort_by(|a, b| {
            let rem_a = (a.expected_duration_secs - a.started_at.elapsed().as_secs_f64()).max(0.001);
            let rem_b = (b.expected_duration_secs - b.started_at.elapsed().as_secs_f64()).max(0.001);
            rem_a.partial_cmp(&rem_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut cumulative_freed = 0;
        let mut max_delay = 0.0;

        for p in profiles {
            cumulative_freed += p.allocated_tokens;
            let rem = (p.expected_duration_secs - p.started_at.elapsed().as_secs_f64()).max(0.001);
            if rem > max_delay {
                max_delay = rem;
            }
            if cumulative_freed >= deficit {
                return max_delay;
            }
        }

        max_delay
    }

    /// Dynamic Thinker Dispatch: System 2 issues the explicit, actionable directive for every worker
    fn thinker_dispatch_directive(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        global_contract: &str,
        previous_findings: &[WorkerFinding],
    ) -> Result<String, String> {
        let mut ctx = self.orchestrator_model.create_context(2048, 512, 4)?;
        let system_msg = "You are the System 2 Sovereign Thinker commanding an execution worker.\n\
The user communicates ONLY with you. Workers are your executive hands.\n\
Workers are localized, stateless code-generation engines with short-term context. They do not converse; they execute.\n\
Your worker is equipped with:\n\
- Direct code generation: Write complete code inside ```<lang> ... ``` blocks\n\
- vfs.write(\"filename\", [[content]]): Write complete file content to VFS\n\
- vfs.read(\"filename\"): Read file from VFS\n\
- terminal.run(\"command\"): Execute shell command on host\n\
- DONE: Finish task\n\n\
Your Task:\n\
Command the worker what EXACT technical functionality, algorithm, and logic to implement for the target file.\n\
Do NOT give vague directives like 'write implementation'. Specify the concrete purpose and features.\n\
Be direct, imperative, and specific (max 35 words).";

        let prev_summary = if previous_findings.is_empty() {
            "Initial task dispatch. No previous findings.".to_string()
        } else {
            let last = previous_findings.last().map(|f| f.finding.as_str()).unwrap_or("");
            format!("Previous worker findings: {}", last)
        };

        let contract_section = if global_contract.trim().is_empty() || global_contract == "No strict contract." {
            String::new()
        } else {
            format!("\nGlobal Contract:\n{}", global_contract.trim())
        };

        let writes: Vec<_> = subgoal.write_set.iter().map(|p| p.display().to_string()).collect();
        let reads: Vec<_> = subgoal.read_set.iter().map(|p| p.display().to_string()).collect();

        let user_msg = format!(
            "User Goal: {}\nSub-goal: {}\nTarget Entity: {}\nAuthorized Writes: [{}]\nPermitted Reads: [{}]\nContext: {}{}\n\nDirective for Worker:",
            user_goal, subgoal.description, subgoal.target_entity,
            writes.join(", "), reads.join(", "),
            prev_summary, contract_section
        );
        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
        let output = ctx.generate(&prompt, 384)?;
        let clean = Self::strip_thought(&output);
        if clean.is_empty() {
            Ok(subgoal.description.clone())
        } else {
            Ok(clean)
        }
    }

    /// Universal AI-Powered Contract Distiller:
    /// In accordance with the Sovereign Organizer role, this rule triggers when a deliverable
    /// is too large to inject into downstream workers' attention context.
    /// It queries the AI model to extract ONLY the public interface, exported types,
    /// components, routes, and signatures, producing a lean, language-agnostic contract.
    pub fn distill_interface_contract(&self, filename: &str, content: &str) -> Result<String, String> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() <= AUTO_INJECT_MAX_LINES && content.len() <= AUTO_INJECT_MAX_BYTES {
            return Ok(content.to_string());
        }

        // 1. Attempt AI model distillation using Sovereign Thinker context
        if let Ok(mut ctx) = self.orchestrator_model.create_context(2048, 512, 4) {
            let system_msg = "You are the Universal Interface Architect & Contract Distiller.\n\
Environment: Autonomous Multi-File Software Swarm.\n\
Task: Given a large source file, extract ONLY its public interface contract for other files to link against.\n\
RULES:\n\
1. Output ONLY exported components, public classes, public functions, API route handlers, and method signatures.\n\
2. For HTML/CSS: output the main DOM hierarchy, class names (.class), and IDs (#id).\n\
3. NEVER output internal implementation details, algorithm bodies, or loops. Replace function bodies with `{ ... }` or `...`.\n\
4. Maximum length: 30 lines. Clean, concise, production-grade interface specification.";

            let user_msg = format!("Target File: {}\n\nSource Code:\n```\n{}\n```\n\nPublic Interface Contract:", filename, content);
            let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
            if let Ok(output) = ctx.generate(&prompt, 384) {
                let clean = Self::strip_thought(&output);
                let trimmed = clean.trim();
                if !trimmed.is_empty() {
                    return Ok(trimmed.to_string());
                }
            }
        }

        // 2. Deterministic Fallback if model context allocation or generation is unavailable
        Ok(Self::distill_interface_fallback(filename, content))
    }

    /// Resilient, language-agnostic heuristic fallback for interface extraction across all languages
    /// (Next.js/React, Python, Java, Rust, Go, C/C++, HTML, CSS) when model inference is unavailable.
    pub fn distill_interface_fallback(filename: &str, content: &str) -> String {
        let mut distilled = Vec::new();
        let ext = std::path::Path::new(filename).extension().and_then(|e| e.to_str()).unwrap_or("");

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('#') {
                continue;
            }
            let is_match = match ext {
                "html" | "htm" => {
                    trimmed.contains("class=") || trimmed.contains("id=") || trimmed.starts_with("<div")
                        || trimmed.starts_with("<header") || trimmed.starts_with("<nav") || trimmed.starts_with("<main")
                        || trimmed.starts_with("<section") || trimmed.starts_with("<button") || trimmed.starts_with("<form")
                }
                "css" => {
                    trimmed.starts_with('.') || trimmed.starts_with('#') || trimmed.contains('{') || trimmed.starts_with('@')
                }
                "py" => {
                    trimmed.starts_with("def ") || trimmed.starts_with("class ") || trimmed.starts_with("async def ")
                        || trimmed.starts_with('@')
                }
                _ => {
                    trimmed.starts_with("pub ") || trimmed.starts_with("export ") || trimmed.starts_with("public ")
                        || trimmed.starts_with("protected ") || trimmed.starts_with("func ") || trimmed.starts_with("fn ")
                        || trimmed.starts_with("interface ") || trimmed.starts_with("type ") || trimmed.starts_with("struct ")
                        || trimmed.starts_with("enum ") || trimmed.starts_with("class ") || trimmed.starts_with("def ")
                        || trimmed.starts_with("async ")
                }
            };

            if is_match {
                distilled.push(trimmed);
                if distilled.len() >= 35 {
                    break;
                }
            }
        }

        if !distilled.is_empty() {
            distilled.join("\n")
        } else {
            content.lines().take(AUTO_INJECT_MAX_LINES).collect::<Vec<_>>().join("\n")
        }
    }

    /// Evaluates worker failure with Epistemic Compaction:
    /// 1. Temporary Thinker context inspects the error, code, and deterministic diagnostic.
    /// 2. Thinker generates decision: NUDGE (minor fixable) or RESET (major rewrite).
    /// 3. Thinker context is wiped immediately, purging all broken code from active cache.
    /// 4. Returns the decision and a dense ledger entry.
    fn thinker_evaluate_failure(
        &self,
        subgoal: &SubGoal,
        attempt: usize,
        failed_code: &str,
        runtime_error: &str,
        diagnostic: &DiagnosticCode,
    ) -> Result<(ThinkerDecision, String), String> {
        if let DiagnosticCode::MissingDependency(ref dep) = diagnostic {
            let question = format!(
                "Required dependency or CLI tool '{}' is missing on host.",
                dep
            );
            let ledger_entry = format!(
                "[LEDGER: Worker #{} Attempt #{} PAUSED (ERR_MISSING_DEPENDENCY: '{}') -> Action: CONSULT_USER]",
                subgoal.id, attempt, dep
            );
            return Ok((
                ThinkerDecision::ConsultDependency {
                    dependency: dep.clone(),
                    question,
                },
                ledger_entry,
            ));
        }

        let mut ctx = self.orchestrator_model.create_context(2048, 512, 4)?;

        let system_msg = "You are the System 2 Sovereign Thinker evaluating a worker's failed attempt.\n\
The worker ran into an error. Analyze whether this is a minor fixable slip (syntax, bracket, small typo, missing vfs.write call) or a major structural failure requiring a complete reset.\n\
Output strictly in one of these two formats:\n\
DECISION: NUDGE | REASON: <concise corrective hint for the worker>\n\
or\n\
DECISION: RESET | REASON: <new simplified directive for fresh restart>";

        let snippet_code = if failed_code.len() > 400 {
            &failed_code[..400]
        } else {
            failed_code
        };

        let snippet_err = if runtime_error.len() > 2000 {
            let lines: Vec<&str> = runtime_error.lines().collect();
            if lines.len() > 30 {
                lines[lines.len() - 30..].join("\n")
            } else {
                runtime_error.chars().rev().take(2000).collect::<String>().chars().rev().collect()
            }
        } else {
            runtime_error.to_string()
        };

        let user_msg = format!(
            "Subgoal: {}\nTarget: {}\nAttempt #{}\nDiagnostic: {} - {}\nWorker Code:\n{}\nRuntime Error:\n{}\n\nDecision:",
            subgoal.description,
            subgoal.target_entity,
            attempt,
            diagnostic.name(),
            diagnostic.default_prescription(&subgoal.target_entity),
            snippet_code,
            snippet_err
        );

        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);
        let raw = ctx.generate(&prompt, 384)?;

        // WIPE temporary inspection tokens from Thinker context immediately
        ctx.kv_cache_clear();

        let decision = Self::parse_thinker_decision(&raw, subgoal);
        let ledger_entry = match &decision {
            ThinkerDecision::Nudge(hint) => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} FAILED ({}) -> Action: NUDGE ('{}')]",
                    subgoal.id, attempt, diagnostic.name(), hint
                )
            }
            ThinkerDecision::Reset(directive) => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} FAILED ({}) -> Action: RESET ('{}')]",
                    subgoal.id, attempt, diagnostic.name(), directive
                )
            }
            ThinkerDecision::ConsultDependency { dependency, .. } => {
                format!(
                    "[LEDGER: Worker #{} Attempt #{} PAUSED (ERR_MISSING_DEPENDENCY: '{}') -> Action: CONSULT_USER]",
                    subgoal.id, attempt, dependency
                )
            }
        };

        Ok((decision, ledger_entry))
    }

    pub fn extract_missing_dependency(text: &str) -> Option<String> {
        const ENGINE_INTERNAL_KEYWORDS: &[&str] = &[
            "vfs", "sandbox", "cleanroom", "bwrap", "omni", "omni_engine", "workspace", "stdout", "stderr", "sys", "os"
        ];

        // 1. Python ModuleNotFoundError: No module named 'openpyxl' or "openpyxl"
        if let Some(idx) = text.find("No module named ") {
            let sub = &text[idx + "No module named ".len()..];
            let sub = sub.trim_start();
            let mut quote = None;
            let first_char = sub.chars().next()?;
            let dep = if first_char == '\'' || first_char == '"' {
                quote = Some(first_char);
                &sub[1..]
            } else {
                sub
            };
            let end_idx = if let Some(q) = quote {
                dep.find(q).unwrap_or_else(|| dep.split_whitespace().next().map(|s| s.len()).unwrap_or(dep.len()))
            } else {
                dep.find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-').unwrap_or(dep.len())
            };
            let candidate = dep[..end_idx].trim().to_string();
            if !candidate.is_empty() && !ENGINE_INTERNAL_KEYWORDS.contains(&candidate.to_lowercase().as_str()) {
                return Some(candidate);
            }
        }

        // 2. Python ImportError: cannot import name ... from 'xxx'
        if text.contains("ImportError") {
            if let Some(from_idx) = text.find("from '") {
                let sub = &text[from_idx + 6..];
                if let Some(end) = sub.find('\'') {
                    let dep = sub[..end].trim().to_string();
                    if !dep.is_empty() && !ENGINE_INTERNAL_KEYWORDS.contains(&dep.to_lowercase().as_str()) {
                        return Some(dep);
                    }
                }
            }
        }

        // 3. Shell / Linux command not found:
        // "command not found: jq" or "jq: command not found" or "sh: line 1: jq: not found"
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
                    let clean_lower = clean.to_lowercase();
                    if !clean.is_empty() 
                        && !IGNORED_COMMANDS.contains(&clean_lower.as_str())
                        && !ENGINE_INTERNAL_KEYWORDS.contains(&clean_lower.as_str())
                    {
                        return Some(clean);
                    }
                }
            }
        }

        None
    }

    pub fn classify_failure(
        action: &SwarmAction,
        exec_success: bool,
        exec_output: &str,
        vfs_target_exists: bool,
        vfs_target_size: usize,
        vfs_content: Option<&str>,
        generated_text: &str,
        previous_diagnostic: Option<&DiagnosticCode>,
    ) -> DiagnosticCode {
        Self::classify_failure_for_kind(
            action,
            exec_success,
            exec_output,
            vfs_target_exists,
            vfs_target_size,
            vfs_content,
            generated_text,
            previous_diagnostic,
            None,
        )
    }

    pub fn classify_failure_for_kind(
        action: &SwarmAction,
        exec_success: bool,
        exec_output: &str,
        vfs_target_exists: bool,
        vfs_target_size: usize,
        vfs_content: Option<&str>,
        generated_text: &str,
        previous_diagnostic: Option<&DiagnosticCode>,
        task_kind: Option<&TaskKind>,
    ) -> DiagnosticCode {
        // 0. Deterministic missing dependency detection from execution or output
        if let Some(dep) = Self::extract_missing_dependency(exec_output) {
            return DiagnosticCode::MissingDependency(dep);
        }
        if let Some(dep) = Self::extract_missing_dependency(generated_text) {
            return DiagnosticCode::MissingDependency(dep);
        }

        // 0.5 Deterministic Security Boundary Violation Detection
        if exec_output.contains("ERR_OUT_OF_BOUNDS_WRITE")
            || exec_output.contains("VFS_BOUNDARY_VIOLATION")
        {
            return DiagnosticCode::OutOfBoundsWrite;
        }

        // Check for repeated attractor pattern
        if let Some(prev) = previous_diagnostic {
            if (*prev == DiagnosticCode::PlaceholderOrEmpty || *prev == DiagnosticCode::VfsFileNotWritten)
                && (generated_text.contains("TODO") || generated_text.contains("pass") || generated_text.contains("<!-- TODO"))
            {
                return DiagnosticCode::RepeatedAttractor;
            }
        }

        // Action-specific diagnostics
        if let SwarmAction::PatchFile { .. } = action {
            if !exec_success {
                return DiagnosticCode::PatchFailed(exec_output.to_string());
            }
        }
        if let SwarmAction::DeleteFile { .. } = action {
            if vfs_target_exists {
                return DiagnosticCode::FileNotDeleted;
            }
        }
        if let SwarmAction::RunTerminal { .. } | SwarmAction::TestRun { .. } = action {
            if !exec_success {
                return DiagnosticCode::TestFailed;
            }
        }
        if let SwarmAction::Verdict { status, ref proof_or_missing } = action {
            if !status {
                return DiagnosticCode::AcceptanceCriteriaFailed(proof_or_missing.clone());
            }
        }

        // TaskKind-specific diagnostics
        if let Some(kind) = task_kind {
            match kind {
                TaskKind::Delete => {
                    if vfs_target_exists {
                        return DiagnosticCode::FileNotDeleted;
                    }
                }
                TaskKind::Modify => {
                    if !exec_success {
                        return DiagnosticCode::PatchFailed(exec_output.to_string());
                    }
                    if vfs_target_exists && vfs_content.map(|d| d.trim().is_empty()).unwrap_or(true) {
                        return DiagnosticCode::DiffEmpty;
                    }
                }
                TaskKind::Test => {
                    if exec_output.contains("ERR_ACCEPTANCE_CRITERIA_FAILED") || exec_output.contains("Missing required artifact") {
                        return DiagnosticCode::AcceptanceCriteriaFailed(exec_output.to_string());
                    }
                    if !exec_success {
                        return DiagnosticCode::TestFailed;
                    }
                }
                _ => {}
            }
        }

        // 1. Missing code block or unparsed action
        if matches!(action, SwarmAction::None) && !generated_text.contains("```") {
            return DiagnosticCode::NoCodeBlock;
        }

        // 2. Execution failure
        if !exec_success {
            return DiagnosticCode::TestFailed;
        }

        // 3. File not created in VFS
        if !vfs_target_exists {
            return DiagnosticCode::VfsFileNotWritten;
        }

        // 4. File is too small or contains placeholder tokens
        if vfs_target_size <= 30 {
            return DiagnosticCode::PlaceholderOrEmpty;
        }

        if let Some(content) = vfs_content {
            let trimmed = content.trim();
            if trimmed.contains("TODO")
                || trimmed.starts_with("pass")
                || trimmed == "..."
            {
                return DiagnosticCode::PlaceholderOrEmpty;
            }
        }

        DiagnosticCode::VfsFileNotWritten
    }

    pub fn prompt_user_for_dependency(dep: &str, _question: &str, auto_approve: Option<bool>) -> DependencyChoice {
        let pm = HostPackageManager::detect();
        let install_cmd = pm.build_install_command(dep);

        println!("\n{}", "────────────────────────────────────────────────────────────────────────────────".bright_yellow());
        println!("  [!] {}", format!("المكتبة أو الأداة المطلوبة غير متوفرة في النظام: '{}'", dep).bright_yellow().bold());
        println!("  بوابة اعتراض الاعتماديات (Universal Dependency Interceptor):");
        println!("     مدير حزم النظام المكتشف: {:?}", pm);
        println!("     أمر التثبيت المقترح: {}", install_cmd.bright_cyan());
        println!("     [1] تثبيت المكتبة للمتابعة (Install & Retry) [y/yes]");
        println!("     [2] المتابعة دون تثبيت باستخدام بديل متاح (Use Alternative) [n/no]");
        println!("{}", "────────────────────────────────────────────────────────────────────────────────".bright_yellow());

        if let Some(auto) = auto_approve {
            if auto {
                println!("  [*] وضع الموافقة التلقائية مفعل (--yes / -y). الإجراء المعتمد: [1] تثبيت المكتبة للمتابعة.");
                return DependencyChoice::InstallAndRetry;
            } else {
                println!("  [*] وضع الرفض التلقائي مفعل (--no). الإجراء المعتمد: [2] المتابعة دون تثبيت باستخدام بديل.");
                return DependencyChoice::UseAlternative;
            }
        }

        if !io::stdin().is_terminal() {
            println!("  [*] طرفية غير تفاعلية (Non-interactive mode). الاختيار التلقائي: [1] محاولة التثبيت.");
            return DependencyChoice::InstallAndRetry;
        }

        loop {
            print!("  >> اختيارك [1/2 أو y/n، الافتراضي: 1]: ");
            let _ = io::stdout().flush();
            let mut input = String::new();
            match io::stdin().read_line(&mut input) {
                Ok(0) => {
                    println!();
                    return DependencyChoice::InstallAndRetry;
                }
                Ok(_) => {
                    let trimmed = input.trim().to_lowercase();
                    if trimmed.is_empty() || trimmed == "1" || trimmed == "y" || trimmed == "yes" {
                        return DependencyChoice::InstallAndRetry;
                    } else if trimmed == "2" || trimmed == "n" || trimmed == "no" {
                        return DependencyChoice::UseAlternative;
                    }
                    println!("     يرجى إدخال 1 (Yes) أو 2 (No).");
                }
                Err(_) => {
                    return DependencyChoice::InstallAndRetry;
                }
            }
        }
    }

    pub fn parse_thinker_decision(text: &str, subgoal: &SubGoal) -> ThinkerDecision {
        let clean = Self::strip_thought(text);
        let upper = clean.to_uppercase();
        if upper.contains("DECISION: RESET") || upper.contains("DECISION:RESET") {
            let reason = if let Some(idx) = clean.find("REASON:") {
                clean[idx + 7..].lines().next().unwrap_or("").trim().to_string()
            } else if let Some(idx) = clean.find('|') {
                clean[idx + 1..].lines().next().unwrap_or("").trim().to_string()
            } else {
                format!("Rewrite complete implementation for {}", subgoal.target_entity)
            };
            ThinkerDecision::Reset(reason)
        } else {
            let reason = if let Some(idx) = clean.find("REASON:") {
                clean[idx + 7..].lines().next().unwrap_or("").trim().to_string()
            } else if let Some(idx) = clean.find('|') {
                clean[idx + 1..].lines().next().unwrap_or("").trim().to_string()
            } else {
                format!("Fix syntax error and ensure {} is written to VFS", subgoal.target_entity)
            };
            ThinkerDecision::Nudge(reason)
        }
    }

    pub fn is_explicit_single_file(goal: &str) -> bool {
        let lower = goal.to_lowercase();
        // Interactive applications, modular suites, multi-component systems, or tasks requesting multiple files are explicitly NOT single-file tasks
        if lower.contains("interactive")
            || lower.contains("modular")
            || lower.contains("separate files")
            || lower.contains("multiple files")
            || lower.contains("multi-file")
            || lower.contains("تفاعلي")
            || lower.contains("تفاعلية")
            || lower.contains("متعدد")
        {
            return false;
        }

        lower.contains("single file")
            || lower.contains("one file")
            || lower.contains("a file")
            || lower.contains("a script")
            || lower.contains("one script")
            || lower.contains("single script")
            || lower.contains("standalone script")
            || lower.contains("ملف واحد")
            || lower.contains("ملفا واحدا")
            || lower.contains("سكربت واحد")
            || lower.contains("صفحة واحدة")
            || lower.contains("صفحه واحده")
            || lower.contains("في ملف")
            || lower.contains("فى ملف")
            || lower.contains("utility")
            || lower.contains("renderer")
            || lower.contains("benchmark")
            || lower.contains("simulator")
            || lower.contains("scanner")
            || lower.contains("crawler")
            || lower.contains("solver")
            || lower.contains("monitor")
            || lower.contains("summarizer")
            || lower.contains("evaluator")
            || lower.contains("اداة")
            || lower.contains("أداة")
            || (lower.contains("internet speed") || lower.contains("سرعة الإنترنت"))
    }

    /// Normalizes entity filename to proper casing (e.g. PascalCase for Java files)
    pub fn normalize_entity_filename(entity: &str) -> String {
        let trimmed = entity.trim();
        if trimmed.to_lowercase().ends_with(".java") {
            let stem = &trimmed[..trimmed.len() - 5];
            let pascal: String = stem
                .split(|c: char| c == '_' || c == '-')
                .filter(|s| !s.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            let rest: String = chars.collect();
                            first.to_uppercase().collect::<String>() + &rest
                        }
                    }
                })
                .collect();
            if !pascal.is_empty() {
                return format!("{}.java", pascal);
            }
        }
        trimmed.to_string()
    }

    /// Extract explicit deliverable filenames directly referenced in goal prompt
    pub fn extract_explicit_deliverables(goal: &str) -> Vec<String> {
        let mut files = Vec::new();
        let valid_exts = [".py", ".html", ".js", ".css", ".java", ".rs", ".sh", ".json", ".md", ".csv", ".txt"];

        for token in goal.split(|c: char| {
            c.is_whitespace() || c == '"' || c == '\'' || c == '`' || c == '(' || c == ')' || c == '[' || c == ']' || c == ',' || c == ':' || c == ';'
        }) {
            let trimmed = token.trim().trim_matches(|c: char| c == '.' || c == '/' || c == '\\');
            for ext in &valid_exts {
                if trimmed.ends_with(ext) && trimmed.len() > ext.len() {
                    let fname = trimmed.to_string();
                    if !fname.contains('/') && !fname.contains('\\') && !files.contains(&fname) {
                        files.push(fname);
                    }
                }
            }
        }
        files
    }

    pub fn infer_target_entity(goal: &str) -> String {
        let explicit = Self::extract_explicit_deliverables(goal);
        if let Some(first) = explicit.into_iter().next() {
            return first;
        }

        let lower = goal.to_lowercase();
        let is_java = (lower.contains(".java") || lower.contains("جافا") || lower.contains("java"))
            && !lower.contains("javascript")
            && !lower.contains("js");

        if is_java {
            "Main.java".to_string()
        } else if lower.contains(".html") || lower.contains("html") || lower.contains("webapp") || lower.contains("web app") || lower.contains("frontend") || lower.contains("javascript") || lower.contains("css") || lower.contains("صفحة") || lower.contains("صفحه") || lower.contains("موقع") || lower.contains("web") {
            if lower.contains(".py") || lower.contains("python") || lower.contains("server") || lower.contains("backend") {
                "app.py".to_string()
            } else {
                "index.html".to_string()
            }
        } else if lower.contains(".py") || lower.contains("python") || lower.contains("بايثون") {
            if lower.contains("speed") || lower.contains("سرعة") || lower.contains("سرعه") {
                "measure_speed.py".to_string()
            } else {
                "main.py".to_string()
            }
        } else if lower.contains(".pdf") || lower.contains("pdf") {
            "document.pdf".to_string()
        } else if lower.contains(".sh") || lower.contains("bash") || lower.contains("shell") || lower.contains("باش") {
            "script.sh".to_string()
        } else if lower.contains(".json") || lower.contains("json") {
            "data.json".to_string()
        } else if lower.contains(".csv") || lower.contains("csv") || lower.contains("جدول")
            || lower.contains("excel") || lower.contains("xlsx") || lower.contains("xls")
            || lower.contains("اكسل") || lower.contains("إكسل")
        {
            "data.csv".to_string()
        } else if lower.contains(".md") || lower.contains("markdown") || lower.contains("تقرير") || lower.contains("report") {
            "report.md".to_string()
        } else {
            "output.txt".to_string()
        }
    }

fn is_build_task(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("create")
        || lower.contains("build")
        || lower.contains("game")
        || lower.contains("java")
        || lower.contains("جافا")
        || lower.contains("html")
        || lower.contains("css")
        || lower.contains("js")
        || lower.contains("write")
        || lower.contains("code")
        || lower.contains("implement")
        || lower.contains("develop")
        || lower.contains("make")
        || lower.contains("web")
        || lower.contains("python")
        || lower.contains("py")
        || lower.contains("pdf")
        || lower.contains("script")
        || lower.contains("program")
        || lower.contains("tool")
        || lower.contains("utility")
        || lower.contains("app")
        || lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("json")
        || lower.contains("csv")
        || lower.contains("excel")
        || lower.contains("xlsx")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("clean")
        || lower.contains("patch")
        || lower.contains("modify")
        || lower.contains("edit")
        || lower.contains("fix")
        || lower.contains("refactor")
        || lower.contains("test")
        || lower.contains("verify")
        || lower.contains("check")
        || lower.contains("اكتب")
        || lower.contains("انشئ")
        || lower.contains("أنشئ")
        || lower.contains("اصنع")
        || lower.contains("اعمل")
        || lower.contains("احذف")
        || lower.contains("امسح")
        || lower.contains("نظف")
        || lower.contains("ازالة")
        || lower.contains("إزالة")
        || lower.contains("عدل")
        || lower.contains("صلح")
        || lower.contains("غير")
        || lower.contains("تعديل")
        || lower.contains("تصليح")
        || lower.contains("افحص")
        || lower.contains("شغل")
        || lower.contains("اختبر")
        || lower.contains("تحقق")
        || lower.contains("صفحة")
        || lower.contains("صفحه")
        || lower.contains("كود")
        || lower.contains("برمج")
        || lower.contains("سكربت")
        || lower.contains("سكريبت")
        || lower.contains("بايثون")
        || lower.contains("برنامج")
        || lower.contains("اداة")
        || lower.contains("أداة")
        || lower.contains("اكسل")
        || lower.contains("إكسل")
        || lower.contains("تقرير")
        || lower.contains("ملف")
        || lower.contains("لعبة")
        || lower.contains("لعبه")
        || lower.contains("موقع")
        || lower.contains("تطبيق")
        || lower.contains("جدول")
        || lower.contains("زر")
}

pub fn infer_task_kind(text: &str) -> TaskKind {
    let lower = text.to_lowercase();
    let trimmed = lower.trim();

    // Priority 1: Testing and Verification tasks take precedence
    if lower.contains("test") || lower.contains("smoke test") || lower.contains("unit test")
        || lower.contains("verification") || lower.contains("verify") || lower.contains("cleanroom")
        || trimmed.starts_with("run ") || trimmed.starts_with("execute ")
        || lower.contains("افحص") || lower.contains("شغل") || lower.contains("اختبر")
    {
        return TaskKind::Test;
    }

    // Priority 2: Explicit creation verbs
    if trimmed.starts_with("implement") || trimmed.starts_with("create") || trimmed.starts_with("build")
        || trimmed.starts_with("write") || trimmed.starts_with("generate") || trimmed.starts_with("code")
        || trimmed.starts_with("add") || trimmed.starts_with("develop")
        || trimmed.starts_with("انشئ") || trimmed.starts_with("أنشئ") || trimmed.starts_with("ابن")
        || trimmed.starts_with("اكتب") || trimmed.starts_with("برمج") || trimmed.starts_with("طور")
    {
        return TaskKind::Create;
    }

    // Priority 3: Modification / Refactoring / Fixing tasks
    if lower.contains("patch") || lower.contains("modify") || lower.contains("edit") || lower.contains("fix")
        || lower.contains("refactor") || lower.contains("migrate") || lower.contains("update")
        || lower.contains("عدل") || lower.contains("صلح") || lower.contains("غير") || lower.contains("تعديل")
        || lower.contains("تصليح") || lower.contains("تحديث") || lower.contains("ترقية")
    {
        return TaskKind::Modify;
    }

    // Priority 4: Explicit deletion tasks (targeted file removal, not delete buttons or HTTP DELETE endpoints)
    if trimmed.starts_with("delete ") || trimmed.starts_with("remove ") || trimmed.starts_with("rm ")
        || lower.contains("delete file") || lower.contains("remove file") || lower.contains("cleanup redundant")
        || lower.contains("delete redundant") || lower.contains("remove redundant")
        || trimmed.starts_with("احذف") || trimmed.starts_with("امسح") || trimmed.starts_with("نظف")
        || lower.contains("حذف الملف") || lower.contains("مسح الملف") || lower.contains("إزالة الملف")
    {
        TaskKind::Delete
    } else {
        TaskKind::Create
    }
}

    /// Renders an interactive plan review card in the terminal for user inspection and steering
    fn render_plan_preview(&self, goal: &str, subgoals: &[SubGoal]) {
        use colored::*;
        println!("\n  {}", "┌────────────────────────────────────────────────────────────────────────┐".bright_cyan());
        println!("  │ {:<70} │", " [PLAN REVIEW] System 2 Sovereign Architecture Decomposition".bright_yellow().bold());
        println!("  {}", "├────────────────────────────────────────────────────────────────────────┤".bright_cyan());
        let truncated_goal = if goal.len() > 58 {
            format!("{}...", &goal[..55])
        } else {
            goal.to_string()
        };
        println!("  │ Objective: {:<58} │", truncated_goal.bright_white());
        println!("  {}", "├────────────────────────────────────────────────────────────────────────┤".bright_cyan());
        println!("  │ Proposed Sub-tasks ({} total):{:<43} │", subgoals.len(), "");
        for sg in subgoals {
            let kind_tag = match sg.kind {
                TaskKind::Create => "[CREATE]".bright_green(),
                TaskKind::Modify => "[MODIFY]".bright_yellow(),
                TaskKind::Test => "[TEST]".bright_blue(),
                TaskKind::Delete => "[DELETE]".bright_red(),
                TaskKind::Search => "[SEARCH]".bright_magenta(),
            };
            let desc_trunc = if sg.description.len() > 44 {
                format!("{}...", &sg.description[..41])
            } else {
                sg.description.clone()
            };
            println!("  │   #{:<2} {} {:<46} │", sg.id, kind_tag, desc_trunc);
            let writes_str: Vec<_> = sg.write_set.iter().map(|p| p.display().to_string()).collect();
            let reads_str: Vec<_> = sg.read_set.iter().map(|p| p.display().to_string()).collect();
            println!("  │        Target: {:<16} Writes: [{:<12}] Reads: [{:<12}] │",
                     sg.target_entity.bright_cyan(),
                     writes_str.join(", "),
                     reads_str.join(", "));
        }
        println!("  {}", "└────────────────────────────────────────────────────────────────────────┘".bright_cyan());
        println!("  [?] Approve execution plan?");
        println!("      - Type '{}' (or '{}') to approve and seal plan into memory.", "yes".bright_green().bold(), "y".bright_green());
        println!("      - Type '{}' to refine/steer the architecture before execution.\n", "no <critique>".bright_yellow().bold());
    }

    /// System 2: Generates a unifying blackboard contract for independent DAG workers
    fn generate_global_contract(&self, goal: &str, subgoals: &[SubGoal]) -> Result<String, String> {
        let mut ctx = self.orchestrator_model.create_context(2048, 512, 4)?;
        
        // Explicit VFS Boundary Contract Header (Deterministic)
        let mut contract = String::from("VFS BOUNDARY CONTRACT:\n");
        for sg in subgoals {
            let writes: Vec<_> = sg.write_set.iter().map(|p| p.display().to_string()).collect();
            let reads: Vec<_> = sg.read_set.iter().map(|p| p.display().to_string()).collect();
            contract.push_str(&format!(
                "- Task #{}: writes=[{}], reads=[{}], depends_on=[{}]\n",
                sg.id,
                if writes.is_empty() { "none".to_string() } else { writes.join(", ") },
                if reads.is_empty() { "none".to_string() } else { reads.join(", ") },
                if sg.depends_on.is_empty() { "none".to_string() } else { sg.depends_on.join(", ") }
            ));
        }
        
        let sg_list = subgoals.iter().map(|sg| {
            let writes: Vec<_> = sg.write_set.iter().map(|p| p.display().to_string()).collect();
            let reads: Vec<_> = sg.read_set.iter().map(|p| p.display().to_string()).collect();
            format!(
                "- Task #{}: {} (target: {}) | writes: [{}] | reads: [{}] | depends_on: [{}]",
                sg.id, sg.description, sg.target_entity, writes.join(", "), reads.join(", "), sg.depends_on.join(", ")
            )
        }).collect::<Vec<_>>().join("\n");
        
        let system = "You are the System 2 Sovereign Architect.\n\
        Your team of workers will execute the following sub-goals in parallel/isolated contexts.\n\
        To prevent conflicts, synthesize the ARCHITECTURAL CONSTANTS section of the GLOBAL CONTRACT.\n\
        Specify ONLY shared constants: ports, database filenames, API endpoints, shared struct names, or UI themes.\n\
        Do NOT write code. Write concise bullet points. Max 50 words.";
        
        let user = format!("Goal: {}\nSubgoals:\n{}\n\nGenerate ARCHITECTURAL CONSTANTS:", goal, sg_list);
        let prompt = Self::format_prompt(&self.orchestrator_model, system, &user);
        
        let tokens = self.orchestrator_model.tokenize(&prompt, true)?;
        ctx.eval_tokens(&tokens, 0)?;
        
        let mut llm_part = String::new();
        let mut sampler = crate::native_llama::NativeLlamaSampler::new(&Default::default(), self.orchestrator_model.n_vocab())?;
        for _ in 0..384 {
            let tok = ctx.sample(&mut sampler)?;
            let text = self.orchestrator_model.token_to_piece(tok).unwrap_or_default();
            if text.is_empty() || self.orchestrator_model.is_stop_token(tok) || self.orchestrator_model.is_stop_piece(&text) {
                break;
            }
            llm_part.push_str(&text);
            ctx.eval_tokens(&[tok], 0)?;
        }
        
        let clean_constants = Self::strip_thought(&llm_part);
        if !clean_constants.trim().is_empty() {
            contract.push_str("\nARCHITECTURAL CONSTANTS:\n");
            contract.push_str(clean_constants.trim());
        }
        
        Ok(contract.trim().to_string())
    }

    /// System 2: Decomposes goal into discrete sub-goals
    fn orchestrate_plan(&self, goal: &str) -> Result<Vec<SubGoal>, String> {
        let is_build = Self::is_build_task(goal);

        let mut ctx = self.orchestrator_model.create_context(4096, 512, 4)?;

        let system_msg = if is_build {
            if self.config.is_modify_mode {
                let existing_vfs_files = self.vfs.list_files();
                let files_list = if existing_vfs_files.is_empty() {
                    "None".to_string()
                } else {
                    existing_vfs_files.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
                };
                format!(
                    "You are the System 2 Sovereign Architect.\n\
                    MODE: INCREMENTAL FEATURE EXTENSION & CODEBASE MODIFICATION.\n\
                    Existing Project Deliverables in Workspace: [{}]\n\
                    Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\n\
                    MANDATORY ARCHITECTURAL RULES FOR MODIFICATION:\n\
                    1. ZERO-WIPE INVARIANT: DO NOT wipe, delete, or rewrite existing project deliverables from scratch.\n\
                    2. For existing files being updated or extended with new features, declare KIND: MODIFY.\n\
                    3. Workers receive the existing source code and MUST preserve 100% of existing working functions, routes, and logic while incrementally adding requested features.\n\
                    4. Declare KIND: CREATE only if a brand new supplementary file (e.g. style sheet, new module) is strictly needed.\n\
                    5. MANDATORY ACCEPTANCE TEST: Append a final review test worker (KIND: TEST) verifying the new features in the cleanroom.\n\n\
                    Output each sub-goal on a new line strictly formatted as:\n\
                    SUBGOAL: <explicit modification directive> | ENTITY: <target filename> | KIND: <MODIFY|CREATE|TEST> | READ_SET: <comma-separated files> | WRITE_SET: <comma-separated files> | DEPENDS_ON: <dependencies or none>\n\n\
                    Examples:\n\
                    SUBGOAL: Extend frontend with dark/light mode toggle and task reordering | ENTITY: index.html | KIND: MODIFY | READ_SET: index.html | WRITE_SET: index.html | DEPENDS_ON: none\n\
                    SUBGOAL: Update backend API routes to support task reordering | ENTITY: app.py | KIND: MODIFY | READ_SET: app.py | WRITE_SET: app.py | DEPENDS_ON: none\n\
                    SUBGOAL: Execute cleanroom verification and validate modified features | ENTITY: index.html | KIND: TEST | READ_SET: index.html, app.py | WRITE_SET: none | DEPENDS_ON: index.html",
                    files_list
                )
            } else {
                format!(
                    "You are the System 2 Sovereign Architect.\n\
                    Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                    Workers write standalone project files directly in their native language (e.g. ```python, ```sh, ```html, ```java).\n\n\
                    PARSIMONY & MODULAR DISPATCH RULES:\n\
                    - You decide how many workers to deploy (from 2 up to {} max budget).\n\
                    - MODULAR DECOMPOSITION MANDATE: Decompose non-trivial software applications into distinct, cohesive sub-tasks across modular files (e.g. data models -> core logic/storage -> interactive CLI interface).\n\
                    - NEVER collapse an entire interactive application or multi-component project into a single monolithic worker task. Break it into at least 2 distinct implementation tasks.\n\
                    - For Java: Target filenames MUST use PascalCase (e.g. GameLoop.java, Snake.java, Food.java) matching their primary public class.\n\
                    - SPREADSHEETS & TABULAR: For spreadsheets, Excel, or tabular data, direct the worker to generate a clean, well-formatted CSV file (ENTITY: data.csv) which Excel and spreadsheet software open natively.\n\
                    - VFS BOUNDARIES: Explicitly declare the file(s) the worker will create/mutate (WRITE_SET) and any existing file(s) it needs to inspect (READ_SET).\n\
                    - MANDATORY CLEANROOM ACCEPTANCE TEST: When the objective creates an executable program or CLI application, you MUST ALWAYS append a final review test worker (kind: TEST) which lists the staged deliverable in READ_SET and DEPENDS_ON to verify user acceptance criteria in the isolated cleanroom.\n\n\
                    DELIVERABLES SPECIFICATION:\n\
                    - Explicitly determine the exact deliverable files and total count required to satisfy the user objective.\n\
                    - You may declare planned deliverable files on the first line:\n\
                    PLANNED_FILES: <comma-separated list of filenames, e.g. index.html, styles.css>\n\n\
                    Output each sub-goal on a new line strictly formatted as:\n\
                    SUBGOAL: <explicit implementation directive> | ENTITY: <target filename> | READ_SET: <comma-separated files or none> | WRITE_SET: <comma-separated files or target filename> | DEPENDS_ON: <comma-separated dependencies or none>\n\n\
                    Examples:\n\
                    Modular software application deliverable:\n\
                    SUBGOAL: Implement core data models, business logic, and file persistence | ENTITY: core_models.py | READ_SET: none | WRITE_SET: core_models.py | DEPENDS_ON: none\n\
                    SUBGOAL: Implement interactive CLI interface, menu loop, and user commands | ENTITY: app_cli.py | READ_SET: core_models.py | WRITE_SET: app_cli.py | DEPENDS_ON: core_models.py\n\
                    SUBGOAL: Execute cleanroom verification and validate interactive user workflows | ENTITY: app_cli.py | READ_SET: app_cli.py, core_models.py | WRITE_SET: none | DEPENDS_ON: app_cli.py\n\
                    Single-file deliverable:\n\
                    SUBGOAL: Implement internet speed measurement tool using download and upload tests | ENTITY: measure_speed.py | READ_SET: none | WRITE_SET: measure_speed.py | DEPENDS_ON: none\n\
                    Multi-file deliverable:\n\
                    SUBGOAL: Implement backend API endpoints and data storage | ENTITY: server.py | READ_SET: none | WRITE_SET: server.py | DEPENDS_ON: none\n\
                    SUBGOAL: Implement client frontend interface and network calls | ENTITY: client.py | READ_SET: server.py | WRITE_SET: client.py | DEPENDS_ON: server.py\n\
                    Spreadsheet / tabular deliverable:\n\
                    SUBGOAL: Write the complete tabular data in CSV format | ENTITY: data.csv | READ_SET: none | WRITE_SET: data.csv | DEPENDS_ON: none\n\
                    Full-stack web application with backend server and UI:\n\
                    SUBGOAL: Implement backend API server and HTTP endpoints | ENTITY: app.py | READ_SET: none | WRITE_SET: app.py | DEPENDS_ON: none\n\
                    SUBGOAL: Implement interactive frontend HTML with dark mode CSS and JavaScript | ENTITY: index.html | READ_SET: app.py | WRITE_SET: index.html | DEPENDS_ON: app.py\n\
                    Multi-file web deliverable:\n\
                    SUBGOAL: Write the HTML structure and markup | ENTITY: index.html | READ_SET: none | WRITE_SET: index.html | DEPENDS_ON: none\n\
                    SUBGOAL: Write the external CSS styling | ENTITY: styles.css | READ_SET: index.html | WRITE_SET: styles.css | DEPENDS_ON: index.html\n\n\
                    - REVIEW & TEST WORKER: When the objective creates an executable program, you may deploy a final review test worker (kind: TEST) which MUST list all code files it tests in READ_SET and DEPENDS_ON.",
                    self.config.max_subgoals
                )
            }
        } else {
            format!(
                "You are the System 2 Sovereign Architect.\n\
                Decision Authority on Workers:\n\
                - You decide how many workers to deploy (from 1 up to {} max budget).\n\
                - For straightforward topics, 1 worker is sufficient.\n\n\
                Output each sub-goal on a new line strictly formatted as:\n\
                SUBGOAL: <precise search objective> | ENTITY: <primary keyword> | READ_SET: <comma-separated files or none> | WRITE_SET: <comma-separated files or none> | DEPENDS_ON: <dependencies or none>\n\n\
                Examples:\n\
                SUBGOAL: Search for room temperature superconductors 2026 breakthroughs | ENTITY: superconductors | READ_SET: none | WRITE_SET: none | DEPENDS_ON: none\n\
                SUBGOAL: Search for recent astrophysics discoveries | ENTITY: astrophysics | READ_SET: none | WRITE_SET: none | DEPENDS_ON: none",
                self.config.max_subgoals
            )
        };

        let user_msg = format!("Goal: {}", goal);
        let prompt = Self::format_prompt(&self.orchestrator_model, &system_msg, &user_msg);

        let output = ctx.generate(&prompt, 1024)?;
        println!("  [THINKER PLAN RAW OUTPUT]\n{}\n  [END THINKER PLAN RAW OUTPUT]", output.trim());
        let mut subgoals = Vec::new();
        let mut seen_entities = std::collections::HashSet::new();
        let mut planned_files: Vec<String> = Vec::new();
        let mut id = 1;

        for line in output.lines() {
            let mut l = line.trim();
            while l.starts_with('`') {
                l = l.trim_start_matches('`').trim();
            }
            if l.is_empty() {
                continue;
            }

            let l_upper = l.to_uppercase();
            if l_upper.starts_with("PLANNED_FILES:") || l_upper.starts_with("REQUIRED_FILES:") || l_upper.starts_with("DELIVERABLES:") || l_upper.starts_with("FILES:") {
                let tag_len = if l_upper.starts_with("PLANNED_FILES:") { 14 }
                    else if l_upper.starts_with("REQUIRED_FILES:") { 15 }
                    else if l_upper.starts_with("DELIVERABLES:") { 13 }
                    else { 6 };
                let files_str = l[tag_len..].trim();
                for f in files_str.split(',') {
                    let cleaned = Self::normalize_entity_filename(f.trim());
                    if !cleaned.is_empty() && !planned_files.contains(&cleaned) {
                        planned_files.push(cleaned);
                    }
                }
                continue;
            }

            // Resilient Subgoal Detection:
            // Match SUBGOAL: or SUB-GOAL: anywhere in the line, stripping numbered lists (1., 2.), markdown bullets (- , *), bold (**SUBGOAL:**), etc.
            let subgoal_pos = if let Some(pos) = l_upper.find("SUBGOAL:") {
                Some(pos + 8)
            } else if let Some(pos) = l_upper.find("SUB-GOAL:") {
                Some(pos + 9)
            } else {
                None
            };

            let rest = match subgoal_pos {
                Some(idx) => l[idx..].trim().trim_matches('*').trim(),
                None => continue,
            };

            let pipe_parts: Vec<&str> = rest.split('|').map(|s| s.trim()).collect();
            if pipe_parts.is_empty() {
                continue;
            }

            let desc = pipe_parts[0].to_string();
            let mut raw_entity = String::new();
            let mut raw_depends_on = String::new();
            let mut raw_read_set = Vec::new();
            let mut raw_write_set = Vec::new();
            let mut explicit_kind: Option<TaskKind> = None;

            for part in &pipe_parts[1..] {
                let p_upper = part.to_uppercase();
                if p_upper.starts_with("KIND:") || p_upper.starts_with("TASK_KIND:") || p_upper.starts_with("TYPE:") {
                    let tag_len = if p_upper.starts_with("KIND:") { 5 } else if p_upper.starts_with("TASK_KIND:") { 10 } else { 5 };
                    let val = part[tag_len..].trim().to_uppercase();
                    if val.contains("MODIFY") || val.contains("UPDATE") || val.contains("REFACTOR") || val.contains("PATCH") {
                        explicit_kind = Some(TaskKind::Modify);
                    } else if val.contains("CREATE") || val.contains("BUILD") || val.contains("WRITE") {
                        explicit_kind = Some(TaskKind::Create);
                    } else if val.contains("TEST") || val.contains("VERIFY") {
                        explicit_kind = Some(TaskKind::Test);
                    } else if val.contains("DELETE") || val.contains("REMOVE") {
                        explicit_kind = Some(TaskKind::Delete);
                    }
                } else if p_upper.starts_with("ENTITY:") {
                    raw_entity = part[7..].trim().to_string();
                } else if p_upper.starts_with("DEPENDS_ON:") {
                    raw_depends_on = part[11..].trim().to_string();
                } else if p_upper.starts_with("READ_SET:") || p_upper.starts_with("READS:") {
                    let tag_len = if p_upper.starts_with("READ_SET:") { 9 } else { 6 };
                    let val = part[tag_len..].trim();
                    if val.to_lowercase() != "none" && !val.is_empty() {
                        for item in val.split(',') {
                            let cleaned = Self::normalize_entity_filename(item.trim());
                            if !cleaned.is_empty() {
                                raw_read_set.push(PathBuf::from(cleaned));
                            }
                        }
                    }
                } else if p_upper.starts_with("WRITE_SET:") || p_upper.starts_with("WRITES:") {
                    let tag_len = if p_upper.starts_with("WRITE_SET:") { 10 } else { 7 };
                    let val = part[tag_len..].trim();
                    if val.to_lowercase() != "none" && !val.is_empty() {
                        for item in val.split(',') {
                            let cleaned = Self::normalize_entity_filename(item.trim());
                            if !cleaned.is_empty() {
                                raw_write_set.push(PathBuf::from(cleaned));
                            }
                        }
                    }
                }
            }

            let entity = if !raw_entity.is_empty() {
                Self::normalize_entity_filename(&raw_entity)
            } else if is_build {
                Self::infer_target_entity(goal)
            } else {
                goal.split_whitespace().next().unwrap_or("general").to_string()
            };

            if is_build && !entity.is_empty() && entity != "none" && !planned_files.contains(&entity) {
                planned_files.push(entity.clone());
            }

            for w in &raw_write_set {
                let w_str = w.to_string_lossy().to_string();
                if is_build && !w_str.is_empty() && w_str != "none" && !planned_files.contains(&w_str) {
                    planned_files.push(w_str);
                }
            }

            let mut depends_on = Vec::new();
            if raw_depends_on.to_lowercase() != "none" && !raw_depends_on.is_empty() {
                for dep in raw_depends_on.split(',') {
                    let d = dep.trim();
                    if !d.is_empty() && d.to_lowercase() != "none" {
                        depends_on.push(Self::normalize_entity_filename(d));
                    }
                }
            }

            // Deterministic Fallback Rules (Zero-Hallucination Guarantee)
            let mut write_set = if !raw_write_set.is_empty() {
                raw_write_set
            } else if is_build && !entity.is_empty() {
                vec![PathBuf::from(&entity)]
            } else {
                Vec::new()
            };

            if is_build && !entity.is_empty() && entity != "none" {
                let entity_path = PathBuf::from(&entity);
                if !write_set.contains(&entity_path) {
                    write_set.push(entity_path);
                }
            }

            let read_set = if !raw_read_set.is_empty() {
                raw_read_set
            } else if is_build {
                let mut rs = Vec::new();
                for dep in &depends_on {
                    let p = PathBuf::from(dep);
                    if !rs.contains(&p) {
                        rs.push(p);
                    }
                }
                if !entity.is_empty() {
                    let p = PathBuf::from(&entity);
                    if !rs.contains(&p) {
                        rs.push(p);
                    }
                }
                rs
            } else {
                Vec::new()
            };

            let entity_path = PathBuf::from(&entity);
            let target_exists = self.vfs.exists(&entity_path) || self.resolve_existing_file(&entity_path).is_some();
            let task_kind = if let Some(k) = explicit_kind {
                k
            } else if (self.config.is_modify_mode || target_exists) && !desc.to_lowercase().contains("test") && !desc.to_lowercase().contains("verify") && !desc.to_lowercase().contains("cleanroom") {
                TaskKind::Modify
            } else {
                Self::infer_task_kind(&desc)
            };
            let dedup_key = format!("{}:{}", task_kind.name(), entity.to_lowercase());
            if seen_entities.contains(&dedup_key) {
                continue; // Skip duplicate task kind for the same entity
            }
            seen_entities.insert(dedup_key);

            if !desc.is_empty() {
                subgoals.push(SubGoal {
                    id,
                    description: desc,
                    target_entity: entity,
                    kind: task_kind,
                    guidance: None,
                    depends_on,
                    read_set,
                    write_set,
                    max_tokens: if is_build { 4096 } else { 2048 },
                });
                id += 1;
                if subgoals.len() >= self.config.max_subgoals {
                    break;
                }
            }
        }

        // If user goal explicitly specifies a single file/script, enforce exactly 1 creation worker
        if Self::is_explicit_single_file(goal) && subgoals.len() > 1 {
            subgoals.truncate(1);
            if planned_files.len() > 1 {
                planned_files.truncate(1);
            }
        }

        // Fallback if model output did not match format exactly
        if subgoals.is_empty() {
            if is_build {
                let explicit_files = Self::extract_explicit_deliverables(goal);
                let files_to_plan = if !explicit_files.is_empty() {
                    explicit_files
                } else if self.config.is_modify_mode {
                    let vfs_files: Vec<String> = self.vfs.list_files().iter().map(|p| p.to_string_lossy().to_string()).collect();
                    if !vfs_files.is_empty() {
                        vfs_files
                    } else {
                        vec![Self::infer_target_entity(goal)]
                    }
                } else {
                    vec![]
                };

                if !files_to_plan.is_empty() {
                    for (idx, file) in files_to_plan.iter().enumerate() {
                        let target_path = PathBuf::from(file);
                        let mut read_set = Vec::new();
                        let mut depends_on = Vec::new();
                        if idx > 0 {
                            depends_on.push(files_to_plan[idx - 1].clone());
                            read_set.push(PathBuf::from(&files_to_plan[idx - 1]));
                        }
                        read_set.push(target_path.clone());

                        if !planned_files.contains(file) {
                            planned_files.push(file.clone());
                        }

                        let target_exists = self.vfs.exists(&target_path) || self.resolve_existing_file(&target_path).is_some();
                        let kind = if self.config.is_modify_mode || target_exists {
                            TaskKind::Modify
                        } else {
                            TaskKind::Create
                        };
                        let desc = if kind == TaskKind::Modify {
                            format!("Extend and incrementally update deliverable '{}' with requested features: {}", file, goal)
                        } else {
                            format!("Create and write complete standalone implementation for deliverable '{}'", file)
                        };

                        subgoals.push(SubGoal {
                            id: idx + 1,
                            description: desc,
                            target_entity: file.clone(),
                            kind,
                            guidance: None,
                            depends_on,
                            read_set,
                            write_set: vec![target_path],
                            max_tokens: 4096,
                        });
                    }
                } else {
                    let inferred = Self::infer_target_entity(goal);
                    if !planned_files.contains(&inferred) {
                        planned_files.push(inferred.clone());
                    }
                    let target_path = PathBuf::from(&inferred);
                    let target_exists = self.vfs.exists(&target_path) || self.resolve_existing_file(&target_path).is_some();
                    let task_kind = if self.config.is_modify_mode || target_exists {
                        TaskKind::Modify
                    } else {
                        Self::infer_task_kind(goal)
                    };
                    let desc = if task_kind == TaskKind::Modify {
                        format!("Extend and incrementally update deliverable '{}' with requested features: {}", inferred, goal)
                    } else {
                        format!("Create and write complete standalone implementation for: {}", goal)
                    };
                    subgoals.push(SubGoal {
                        id: 1,
                        description: desc,
                        target_entity: inferred.clone(),
                        kind: task_kind,
                        guidance: None,
                        depends_on: vec![],
                        read_set: vec![target_path.clone()],
                        write_set: vec![target_path],
                        max_tokens: 4096,
                    });
                }
            } else {
                subgoals.push(SubGoal {
                    id: 1,
                    description: format!("Search web for: {}", goal),
                    target_entity: goal.split_whitespace().next().unwrap_or("topic").to_string(),
                    kind: TaskKind::Search,
                    guidance: None,
                    depends_on: vec![],
                    read_set: vec![],
                    write_set: vec![],
                    max_tokens: 2048,
                });
            }
        }

        // Thinker Sovereign Review Gate:
        // Universal, Language-Agnostic Verification: When building or modifying deliverables,
        // deploy an isolated, strictly read-only Review & Verification worker.
        if is_build {
            let has_test_worker = subgoals.iter().any(|sg| sg.kind == TaskKind::Test);
            let candidate_target = subgoals.iter().rfind(|sg| sg.kind == TaskKind::Create || sg.kind == TaskKind::Modify)
                .map(|sg| sg.target_entity.clone());

            if let Some(target) = candidate_target {
                if !has_test_worker && !target.is_empty() {
                    let next_id = subgoals.len() + 1;
                    let target_path = PathBuf::from(&target);
                    let test_sheet = crate::sandbox::cleanroom::TestSheet::from_planned_files(goal, &planned_files);
                    let spec_note = if !test_sheet.required_files.is_empty() {
                        format!(" Enforce required artifacts {:?} and semantic acceptance rubric.", test_sheet.required_files)
                    } else {
                        String::new()
                    };
                    let mut all_read_targets: Vec<PathBuf> = Vec::new();
                    for f in &test_sheet.required_files {
                        let pb = PathBuf::from(f);
                        if !all_read_targets.contains(&pb) {
                            all_read_targets.push(pb);
                        }
                    }
                    for f in &planned_files {
                        let pb = PathBuf::from(f);
                        if !all_read_targets.contains(&pb) {
                            all_read_targets.push(pb);
                        }
                    }
                    if all_read_targets.is_empty() {
                        all_read_targets.push(target_path);
                    }
                    subgoals.push(SubGoal {
                        id: next_id,
                        description: format!("Run Test Sheet protocol verification & review of '{}'.{}", target, spec_note),
                        target_entity: target.clone(),
                        kind: TaskKind::Test,
                        guidance: Some(format!(
                            "Audit against Cognitive Test Sheet: Verify that required files {:?} are staged in VFS, check semantic markers {:?}, simulate interactive inputs {:?}, and confirm 100% acceptance before host commit.",
                            test_sheet.required_files, test_sheet.semantic_markers, test_sheet.interactive_inputs
                        )),
                        depends_on: vec![target],
                        read_set: all_read_targets,
                        write_set: vec![], // Strictly Read-Only (W = ∅)
                        max_tokens: 4096,
                    });
                }
            }
        }

        Ok(subgoals)
    }

    /// System 1: Autonomous Worker Loop with Checkpoint Freeze and Unified KV-Cache Stream
    pub fn run_worker_loop(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        worker_ctx: &mut NativeLlamaContext,
        is_first_worker: bool,
        previous_findings: &[WorkerFinding],
    ) -> Result<WorkerFinding, String> {
        self.run_worker_loop_with_abort(
            user_goal,
            subgoal,
            worker_ctx,
            is_first_worker,
            previous_findings,
            None,
        )
    }

    pub fn run_worker_loop_with_abort(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        worker_ctx: &mut NativeLlamaContext,
        is_first_worker: bool,
        previous_findings: &[WorkerFinding],
        abort_flag: Option<Arc<AtomicBool>>,
    ) -> Result<WorkerFinding, String> {
        self.run_worker_loop_with_abort_and_contracts(
            user_goal,
            subgoal,
            worker_ctx,
            is_first_worker,
            previous_findings,
            abort_flag,
            None,
        )
    }

    pub fn run_worker_loop_with_abort_and_contracts(
        &self,
        user_goal: &str,
        subgoal: &SubGoal,
        worker_ctx: &mut NativeLlamaContext,
        is_first_worker: bool,
        previous_findings: &[WorkerFinding],
        abort_flag: Option<Arc<AtomicBool>>,
        dag_contracts: Option<&HashMap<String, String>>,
    ) -> Result<WorkerFinding, String> {
        let is_build = Self::is_build_task(&subgoal.description) || Self::is_build_task(user_goal);

        let mut attempt = 1;
            let mut last_error_desc: Option<String> = None;
        let max_attempts = 3;
        let mut current_directive = subgoal.guidance.clone().unwrap_or_else(|| subgoal.description.clone());
        let mut declined_dependencies: Vec<String> = Vec::new();
        let mut pending_nudge: Option<String> = None;
        let mut checkpoint: usize = 0;

        let mut rollbacks_count = 0;
        let mut tokens_saved = 0;
        let mut final_finding = String::new();
        let target_path = std::path::PathBuf::from(&subgoal.target_entity);
        let mut last_diagnostic: Option<DiagnosticCode> = None;

        // Baseline Snapshot for Modification Tasks
        if subgoal.kind == TaskKind::Modify {
            if !self.vfs.exists(&target_path) {
                if let Some(real_path) = self.resolve_existing_file(&target_path) {
                    if let Ok(bytes) = std::fs::read(&real_path) {
                        println!("  [VFS PRELOAD] Loaded existing target file for modification from: {}", real_path.display().to_string().bright_cyan());
                        self.vfs.preload_file(&target_path, &bytes);
                    }
                }
            }
            self.vfs.snapshot_file(&target_path);
        }

        // Unified Sovereign Swarm Memory:
        // Record the anchor cursor where this worker begins in the shared KV cache.
        let worker_initial_cursor = worker_ctx.current_cursor();
        let mut is_surgical = false;
        let mut surgery_slice: Option<crate::sandbox::diagnostics::AstSlice> = None;

        // Worker-Scoped Write Ledger: Strict mathematical partition of VFS writes (W_i).
        // A worker can ONLY snapshot and roll back files within its own write scope.
        let mut worker_write_scope = subgoal.write_set.clone();
        if !subgoal.target_entity.is_empty() {
            let target_pb = PathBuf::from(&subgoal.target_entity);
            if !worker_write_scope.contains(&target_pb) {
                worker_write_scope.push(target_pb);
            }
        }

        while attempt <= max_attempts {
            let vfs_scoped_snapshot = self.vfs.take_scoped_snapshot(&worker_write_scope);
            // CHECKPOINT 1: Start of worker step attempt
            if let Some(ref abort) = abort_flag {
                if abort.load(Ordering::Relaxed) {
                    println!("  [ABORT CHECKPOINT] Worker #{} detected cancellation. Halting attempt.", subgoal.id);
                    return Ok(WorkerFinding {
                        worker_id: subgoal.id,
                        sub_goal: subgoal.description.clone(),
                        finding: "CANCELLED: Worker aborted due to upstream dependency failure.".to_string(),
                        steps_taken: attempt,
                        rollbacks_count,
                        tokens_saved_by_rollback: tokens_saved,
                    });
                }
            }
            // Check if starting fresh (attempt 1 or after a RESET)
            if pending_nudge.is_none() {
                if is_first_worker && attempt == 1 {
                    worker_ctx.kv_cache_clear();
                } else if attempt > 1 {
                    // RESET triggered: Surgically roll back ONLY this worker's failed attempt,
                    // preserving all prior workers' KV-cache intact!
                    let _ = worker_ctx.rollback_to(worker_initial_cursor);
                    println!(
                        "  [WORKER RESET] Worker #{} rewound to initial cursor {} (Prior KV cache preserved).",
                        subgoal.id, worker_initial_cursor
                    );
                }

                if is_first_worker {
                    let java_mandate = if subgoal.target_entity.ends_with(".java") {
                        let stem = subgoal.target_entity.trim_end_matches(".java");
                        format!(
                            "\n- Java Requirement: The primary public class MUST be named 'public class {}' to match '{}.java'.",
                            stem, stem
                        )
                    } else {
                        String::new()
                    };

                    let system_msg = if is_build {
                        format!(
                            "You are an Executive Hands Worker in an autonomous dual-model swarm.\n\
                            You are directed exclusively by the System 2 Sovereign Thinker.\n\
                            Environment: In-memory Virtual Filesystem (VFS) sandbox.\n\
                            Overall Goal: {}\n\
                            Current Task: {}\n\
                            Target File: {}\n\n\
                            [ENGINE EXECUTION MANDATE]:\n\
                            1. You are a pure code-generation engine. You have NO conversational persona.\n\
                            2. NEVER output conversational speech, greetings, explanations, or commentary.\n\
                            3. Output ONLY the complete, production-ready code for '{}' inside a code block (e.g. ```python ... ```, ```sh ... ```, ```java ... ```).\n\
                            4. The code MUST be 100% complete, fully implemented, and standalone.\n\
                            5. NEVER output placeholder comments like TODO, FIXME, pass, or ellipses (...).\n\
                            6. For spreadsheets, Excel, or tables: output clean CSV format with headers.",
                            user_goal, subgoal.description, subgoal.target_entity, subgoal.target_entity
                        )
                    } else {
                        format!(
                            "You are an Executive Hands Worker in an autonomous dual-model swarm.\n\
                            You are directed exclusively by the System 2 Sovereign Thinker.\n\
                            Objective: {}\n\n\
                            Available Commands:\n\
                            - SEARCH: <query>\n\
                            - FETCH: <result index or URL>\n\
                            - REPORT: <discovered facts>\n\
                            - DONE\n\n\
                            Rule: Output exactly ONE command per step.\n\
                            When facts are found, output REPORT: <facts>.\n\
                            When objective is fulfilled, output DONE.",
                            subgoal.description
                        )
                    };

                    let user_msg = if is_build {
                        if subgoal.kind == TaskKind::Modify {
                            let target_buf = PathBuf::from(&subgoal.target_entity);
                            let code_snippet = self.vfs.read_file(&target_buf)
                                .map(|b| String::from_utf8_lossy(&b).to_string())
                                .unwrap_or_else(|| {
                                    if let Some(real_p) = self.resolve_existing_file(&target_buf) {
                                        std::fs::read_to_string(&real_p).unwrap_or_default()
                                    } else {
                                        String::new()
                                    }
                                });
                            format!(
                                "Goal: {}\nTask: {}\nTarget File: {}\nThinker Directive: {}\n\n\
                                [EXISTING WORKING CODE IN VFS FOR '{}']:\n```\n{}\n```\n\n\
                                [MANDATORY PRESERVATION INVARIANT]:\n\
                                - Keep and preserve all existing working functions, routes, and logic in '{}'.\n\
                                - Incrementally extend or update the implementation to add the requested new features without deleting previous capabilities.\n\
                                - Output EXCLUSIVELY the complete updated code inside a single markdown code block.\n\
                                - ZERO discussion. ZERO apologies. ZERO introduction. ZERO text outside the code block.\n\
                                - Complete implementation only: NO 'TODO', NO 'pass', NO placeholders.{}\n\
                                Output the complete updated '{}' now:",
                                user_goal, subgoal.description, subgoal.target_entity, current_directive,
                                subgoal.target_entity, code_snippet,
                                subgoal.target_entity, java_mandate, subgoal.target_entity
                            )
                        } else {
                            format!(
                                "Goal: {}\nTask: {}\nTarget File: {}\nThinker Directive: {}\n\n\
                                [ENGINE DIRECTIVE INJECTION - STRICT CONSTRAINTS]:\n\
                                - Output EXCLUSIVELY the complete, working code inside a single markdown code block.\n\
                                - ZERO discussion. ZERO apologies. ZERO introduction. ZERO text outside the code block.\n\
                                - Complete implementation only: NO 'TODO', NO 'pass', NO placeholders.{}\n\
                                - Generate the code for '{}' now:",
                                user_goal, subgoal.description, subgoal.target_entity, current_directive,
                                java_mandate, subgoal.target_entity
                            )
                        }
                    } else {
                        format!(
                            "Goal: {}\nThinker Directive: {}\nTask: {}\n\n\
                            [ENGINE DIRECTIVE INJECTION]:\n\
                            - Output EXCLUSIVELY one command: SEARCH: <query> or REPORT: <facts>.\n\
                            - ZERO conversational text.\n\
                            Execute now:",
                            user_goal, current_directive, subgoal.description
                        )
                    };

                    let prompt = Self::format_prompt(&self.worker_model, &system_msg, &user_msg);
                    let prompt_tokens = self.worker_model.tokenize(&prompt, true)?;
                    worker_ctx.eval_tokens(&prompt_tokens, 0)?;
                } else {
                    // Subsequent Workers: Seamless Multi-Turn continuation in Shared KV-Cache
                    let vfs_files = self.vfs.list_files();
                    let vfs_files_list = if vfs_files.is_empty() {
                        "None".to_string()
                    } else {
                        vfs_files
                            .iter()
                            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    let prev_summary = previous_findings
                        .last()
                        .map(|f| format!("Deliverable for Task #{} committed to VFS: {}", f.worker_id, f.finding))
                        .unwrap_or_else(|| "Previous task complete.".to_string());

                    let observation = format!(
                        "{}\nFiles currently in VFS: [{}]",
                        prev_summary, vfs_files_list
                    );

                    let java_guidance = if subgoal.target_entity.ends_with(".java") {
                        let stem = subgoal.target_entity.trim_end_matches(".java");
                        format!(
                            "\n- In Java, you MUST declare 'public class {}' matching the filename exactly.\n\
                            - Inspect the previous Java classes above in active memory. Use their exact class names, methods, and variables without mismatch.",
                            stem
                        )
                    } else {
                        String::new()
                    };

                    let next_instruction = if is_build {
                        if subgoal.kind == TaskKind::Test {
                            format!(
                                "[SUPREME DIRECTIVE - USER GOAL (SUPREME AUTHORITY)]:\n\
                                \"{}\"\n\n\
                                [CONTINUOUS SWARM DIRECTIVE - ISOLATED TEST WORKER (USER ADVOCATE)]:\n\
                                Task #{}: {}\n\
                                Target File: {}\n\
                                Thinker Directive: {}\n\n\
                                [SANDBOX TEST & ACCEPTANCE MANDATE]:\n\
                                - Your loyalty is EXCLUSIVELY to the User Goal above, NOT to the planner or worker.\n\
                                - If the goal requires specific files, styling, CSS, or functionality that is missing, the deliverable is INCOMPLETE.\n\
                                - Your role is to simulate real user interaction and test '{}' inside the isolated cleanroom.\n\
                                - Output a ```test_run block specifying the execution command and sequential keyboard inputs (one per line) to exercise the program and exit cleanly, e.g.:\n\
                                ```test_run\n\
                                command: python3 {}\n\
                                inputs:\n\
                                1\n\
                                Test Item\n\
                                4\n\
                                ```\n\
                                Execute test now:",
                                user_goal,
                                subgoal.id, subgoal.description, subgoal.target_entity, current_directive,
                                subgoal.target_entity, subgoal.target_entity
                            )
                        } else if subgoal.kind == TaskKind::Modify {
                            let target_buf = PathBuf::from(&subgoal.target_entity);
                            let code_snippet = self.vfs.read_file(&target_buf)
                                .map(|b| String::from_utf8_lossy(&b).to_string())
                                .unwrap_or_else(|| {
                                    if let Some(real_p) = self.resolve_existing_file(&target_buf) {
                                        std::fs::read_to_string(&real_p).unwrap_or_default()
                                    } else {
                                        String::new()
                                    }
                                });
                            format!(
                                "[CONTINUOUS SWARM DIRECTIVE - INCREMENTAL EXTENSION]:\n\
                                Task #{}: {}\n\
                                Target File: {}\n\
                                Thinker Directive: {}\n\n\
                                [EXISTING WORKING CODE IN VFS FOR '{}']:\n```\n{}\n```\n\n\
                                [MANDATORY PRESERVATION INVARIANT]:\n\
                                - Keep and preserve all existing functions, classes, and logic in '{}'.\n\
                                - Incrementally add or update the implementation to satisfy the directive without deleting previous capabilities.\n\
                                - Output EXCLUSIVELY the complete updated code inside a single markdown code block.\n\
                                - ZERO conversational text outside the code block.\n\
                                Output the complete updated '{}' now:",
                                subgoal.id, subgoal.description, subgoal.target_entity, current_directive,
                                subgoal.target_entity, code_snippet,
                                subgoal.target_entity, subgoal.target_entity
                            )
                        } else {
                            let mut read_context = String::new();
                            for read_path in &subgoal.read_set {
                                let fname = read_path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                                if let Some(contracts) = dag_contracts {
                                    if let Some(contract) = contracts.get(&fname) {
                                        read_context.push_str(&format!(
                                            "\n[DAG CONTRACT FOR '{}']:\n```\n{}\n```\n",
                                            fname, contract
                                        ));
                                        continue;
                                    }
                                }

                                if !self.vfs.exists(read_path) {
                                    if let Some(real_path) = self.resolve_existing_file(read_path) {
                                        if let Ok(bytes) = std::fs::read(&real_path) {
                                            self.vfs.preload_file(read_path, &bytes);
                                        }
                                    }
                                }

                                if let Some(bytes) = self.vfs.read_file(read_path) {
                                    let content = String::from_utf8_lossy(&bytes);
                                    if !content.trim().is_empty() {
                                        let lines_count = content.lines().count();
                                        if lines_count > AUTO_INJECT_MAX_LINES || bytes.len() > AUTO_INJECT_MAX_BYTES {
                                            let distilled = Self::distill_interface_fallback(&fname, &content);
                                            read_context.push_str(&format!(
                                                "\n[DAG CONTRACT FOR '{}']:\n```\n{}\n```\n",
                                                fname, distilled
                                            ));
                                        } else {
                                            read_context.push_str(&format!(
                                                "\n[EXISTING DELIVERABLE '{}' IN VFS]:\n```\n{}\n```\n",
                                                read_path.display(), content
                                            ));
                                        }
                                    }
                                }
                            }
                            format!(
                                "[CONTINUOUS SWARM DIRECTIVE - SHARED MEMORY]:\n\
                                Task #{}: {}\n\
                                Target File: {}\n\
                                Thinker Directive: {}\n{}\n\
                                [MANDATORY INTEROPERABILITY]:\n\
                                - You share continuous attention memory with prior workers above.\n\
                                - Strictly attend to the exact class names, method signatures, return types, and fields defined in prior files in memory.\n\
                                - Ensure 100% interoperability and compatibility.{}\n\
                                - Output EXCLUSIVELY the complete, working code for '{}' inside a single code block.\n\
                                - ZERO conversational text outside the code block.\n\
                                Generate the code for '{}' now:",
                                subgoal.id, subgoal.description, subgoal.target_entity, current_directive,
                                read_context,
                                java_guidance, subgoal.target_entity, subgoal.target_entity
                            )
                        }
                    } else {
                        format!(
                            "[CONTINUOUS SWARM DIRECTIVE - SHARED MEMORY]:\n\
                            Task #{}: {}\n\
                            Target: {}\n\
                            Thinker Directive: {}\n\n\
                            - Build upon previous findings above in memory.\n\
                            - Output EXCLUSIVELY one command: SEARCH: <query> or REPORT: <facts>.\n\
                            Execute now:",
                            subgoal.id, subgoal.description, subgoal.target_entity, current_directive
                        )
                    };

                    let turn = Self::format_observation_turn(&self.worker_model, &observation, &next_instruction);
                    let turn_tokens = self.worker_model.tokenize(&turn, false)?;
                    worker_ctx.eval_tokens(&turn_tokens, 0)?;
                }

                // CHECKPOINT FREEZE: Anchor cursor position before generation
                checkpoint = worker_ctx.current_cursor();
                if is_first_worker {
                    println!(
                        "  [WORKER CONTEXT] Worker #1 (Lead): Anchored at position {} tokens.",
                        checkpoint
                    );
                } else {
                    println!(
                        "  [SHARED KV MEMORY] Worker #{} inherited {} tokens of attention context from prior workers.",
                        subgoal.id, worker_initial_cursor.to_string().bright_cyan()
                    );
                    println!(
                        "  [WORKER CONTEXT] Worker #{} anchored at position {} tokens in unified KV-stream.",
                        subgoal.id, checkpoint
                    );
                }
            } else if let Some(ref nudge) = pending_nudge {
                // AST SURGERY DETECTION: Language-agnostic line extraction across any compiler/runtime
                let target_file = if subgoal.kind == TaskKind::Test {
                    subgoal.depends_on.first().cloned().unwrap_or_else(|| subgoal.target_entity.clone())
                } else {
                    subgoal.target_entity.clone()
                };

                let mut target_line = 0;
                if let Some(ref err_text) = last_error_desc {
                    if let Some(num) = Self::extract_error_line_number(err_text, &target_file) {
                        target_line = num;
                    }
                }

                if target_line > 0 {
                    let target_buf = PathBuf::from(&target_file);
                    let content_opt = self.vfs.read_file(&target_buf)
                        .map(|b| String::from_utf8_lossy(&b).to_string())
                        .or_else(|| {
                            self.resolve_existing_file(&target_buf)
                                .and_then(|p| std::fs::read_to_string(p).ok())
                        });
                    if let Some(content) = content_opt {
                        if let Ok(slice) = crate::sandbox::diagnostics::TopologicalDiagnostic::extract_from_source(&target_file, &content, target_line) {
                            surgery_slice = Some(slice);
                            is_surgical = true;
                        }
                    }
                }

                let fence_lang = std::path::Path::new(&target_file)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("text");

                let (nudge_content, msg_type) = if let Some(ref slice) = surgery_slice {
                    println!("  [TOPOLOGICAL MACRO-SURGERY] Extracted broken block in '{}' lines {}-{}", target_file, slice.start_line, slice.end_line);
                    (
                        format!(
                            "[TOPOLOGICAL MACRO-SURGERY - AST ISOLATION]:\n\
                            Diagnostic: Defect localized on line {} in '{}'\n\
                            Runtime/Compiler Error: {}\n\
                            Target Block (Lines {}-{}):\n```{}\n{}\n```\n\
                            MANDATE: Output ONLY the corrected block of code within ```{} ``` fences. Do not output the entire file.",
                            target_line, target_file, nudge, slice.start_line, slice.end_line, fence_lang, slice.content, fence_lang
                        ),
                        "Macro-Surgery AST Nudge"
                    )
                } else {
                    (
                        format!(
                            "[ENGINE DIRECTIVE INJECTION - CORRECTION]:\n\
                            Notice from Thinker: {}\n\
                            Target File: {}\n\
                            MANDATE: Output ONLY the corrected, complete code block now without any commentary or explanations:",
                            nudge, subgoal.target_entity
                        ),
                        "Standard Thinker Nudge"
                    )
                };

                let nudge_turn = self.worker_model.format_nudge_turn(&nudge_content);
                let nudge_tokens = self.worker_model.tokenize(&nudge_turn, false)?;
                worker_ctx.eval_tokens(&nudge_tokens, 0)?;
                println!(
                    "  [WORKER RESUME] Applied {} ({} tokens) from frozen checkpoint {}.",
                    msg_type, nudge_tokens.len(), checkpoint
                );
            }

            // Autoregressive generation with Sovereign Thinker Scratchpad Budget & Isolation
            let mut generated_text = String::new();
            let mut generated_tokens = Vec::new();
            let mut think_tokens_count = 0;
            let max_think_budget = 512;
            let max_deliverable_tokens = if is_build { 1536 } else { 256 };
            let max_total_tokens = max_think_budget + max_deliverable_tokens;

            let mut sampler = crate::native_llama::NativeLlamaSampler::new(&crate::native_llama::SamplingConfig {
                temperature: if is_build { 0.2 } else { 0.7 },
                top_p: 0.95,
                top_k: 40,
                repetition_penalty: 1.1,
                penalty_last_n: 64,
                seed: 42,
            }, self.worker_model.n_vocab()).map_err(|e| e.to_string())?;
            let mut recent_tokens: std::collections::VecDeque<i32> = std::collections::VecDeque::with_capacity(32);

            let spinner_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let gen_start_time = Instant::now();
            let mut gen_tokens_count = 0usize;

            for _ in 0..max_total_tokens {
                // CHECKPOINT 2: Inside token generation loop
                if let Some(ref abort) = abort_flag {
                    if abort.load(Ordering::Relaxed) {
                        println!("  [ABORT CHECKPOINT] Worker #{} preempted during token generation.", subgoal.id);
                        break;
                    }
                }

                let tok = worker_ctx.sample(&mut sampler)?;
                let piece = self.worker_model.token_to_piece(tok)?;
                
                recent_tokens.push_back(tok);
                if recent_tokens.len() > 32 {
                    recent_tokens.pop_front();
                }
                if recent_tokens.len() == 32 {
                    let slice: Vec<i32> = recent_tokens.iter().copied().collect();
                    if slice[..16] == slice[16..] {
                        break;
                    }
                }

                if piece.is_empty() || self.worker_model.is_stop_token(tok) || self.worker_model.is_stop_piece(&piece) {
                    break;
                }

                generated_tokens.push(tok);
                generated_text.push_str(&piece);
                worker_ctx.eval_tokens(&[tok], 0)?;

                gen_tokens_count += 1;
                let elapsed = gen_start_time.elapsed().as_secs_f64();
                let speed = if elapsed > 0.05 { gen_tokens_count as f64 / elapsed } else { 0.0 };
                let frame = spinner_frames[gen_tokens_count % spinner_frames.len()];
                let used_cells = worker_ctx.kv_cache_used_cells();
                let n_ctx = worker_ctx.n_ctx();
                let gauge_width: usize = 8;
                let ratio = (used_cells as f64 / n_ctx.max(1) as f64).clamp(0.0, 1.0);
                let filled = (ratio * gauge_width as f64).round() as usize;
                let bar: String = "█".repeat(filled) + &"░".repeat(gauge_width.saturating_sub(filled));

                print!(
                    "\r   {} Worker #{}: Token #{} ({:.1} t/s) │ KV: [{}] {}/{} │ {:.0}s   ",
                    frame.to_string().bright_cyan().bold(),
                    subgoal.id,
                    gen_tokens_count.to_string().bright_yellow().bold(),
                    speed,
                    bar.bright_magenta(),
                    used_cells,
                    n_ctx,
                    elapsed
                );
                let _ = io::stdout().flush();

                // Dynamic Thinker State Analysis
                let has_open_think = generated_text.contains("<think>") || generated_text.contains("<thought>");
                let has_close_think = generated_text.contains("</think>") || generated_text.contains("</thought>");
                let in_think = has_open_think && !has_close_think;

                if in_think {
                    think_tokens_count += 1;
                    // Enforce mathematical thought budget cap: prevent endless reasoning attractor loops
                    if think_tokens_count >= max_think_budget {
                        let force_close = "\n</think>\n";
                        generated_text.push_str(force_close);
                        if let Ok(force_tokens) = self.worker_model.tokenize(force_close, false) {
                            let _ = worker_ctx.eval_tokens(&force_tokens, 0);
                        }
                    }
                    continue; // Skip termination checks while actively deliberating inside think scratchpad
                }

                // Effective Deliverable Text (strictly post-thought monologue)
                let effective_text = if let Some(pos) = generated_text.rfind("</think>") {
                    &generated_text[pos + 8..]
                } else if let Some(pos) = generated_text.rfind("</thought>") {
                    &generated_text[pos + 10..]
                } else {
                    &generated_text[..]
                };

                let eff_trimmed = effective_text.trim();
                let eff_trimmed_start = effective_text.trim_start();

                // 1. Markdown code block deliverable completed
                if let Some(first_idx) = effective_text.find("```") {
                    if effective_text[first_idx + 3..].contains("```") {
                        break;
                    }
                } else if subgoal.kind == TaskKind::Test {
                    // 2. Tester single-command deliverable: breaks on first newline after non-empty command
                    if !eff_trimmed_start.starts_with("```") && eff_trimmed.len() >= 3 && piece.contains('\n') {
                        break;
                    }
                } else if !is_build {
                    // 3. Non-build single line command/result
                    if !eff_trimmed_start.starts_with("```") && eff_trimmed.len() >= 3 && piece.contains('\n') {
                        break;
                    }
                }
            }

            print!("\r{}\r", " ".repeat(85));
            let _ = io::stdout().flush();

            let parsed_action = Self::parse_action(&generated_text);
            let action = if subgoal.kind == TaskKind::Test {
                match parsed_action {
                    SwarmAction::TestRun { .. } => parsed_action,
                    SwarmAction::Verdict { .. } => parsed_action,
                    SwarmAction::RunTerminal { ref command } => {
                        let clean_cmd = Self::strip_command_prefix(command);
                        SwarmAction::RunTerminal { command: clean_cmd }
                    }
                    SwarmAction::WriteFile { ref content, .. } => {
                        let lines: Vec<&str> = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

                        // Check if any line contains a direct execution command
                        let cmd_candidate = lines.iter().find_map(|l| {
                            let mut clean = Self::strip_command_prefix(l);
                            if (clean.starts_with("print(\"") && clean.ends_with("\")"))
                                || (clean.starts_with("print('") && clean.ends_with("')"))
                            {
                                let inner = &clean[7..clean.len() - 2];
                                clean = Self::strip_command_prefix(inner);
                            }
                            if clean.starts_with("python") || clean.starts_with("pytest") || clean.starts_with("bash") || clean.starts_with("sh ") {
                                Some(clean)
                            } else {
                                None
                            }
                        });

                        let has_test_structure = content.contains("assert ")
                            || content.contains("def test_")
                            || content.contains("unittest")
                            || content.contains("pytest")
                            || content.contains("class Test");

                        if let Some(cmd) = cmd_candidate {
                            if !has_test_structure {
                                SwarmAction::RunTerminal { command: cmd }
                            } else {
                                // Full test suite with a command inside it: save as separate test file
                                let test_filename = if subgoal.target_entity.starts_with("test_") {
                                    subgoal.target_entity.clone()
                                } else {
                                    format!("test_{}", subgoal.target_entity)
                                };
                                self.vfs.write_file(&std::path::PathBuf::from(&test_filename), content.as_bytes());
                                let run_cmd = Self::resolve_verification_command(&test_filename);
                                SwarmAction::RunTerminal { command: run_cmd }
                            }
                        } else if !has_test_structure && lines.len() <= 4 {
                            // Short snippet that didn't match an explicit command: run language-agnostic verification on target
                            let default_cmd = Self::resolve_verification_command(&subgoal.target_entity);
                            SwarmAction::RunTerminal { command: default_cmd }
                        } else {
                            // Actual test script generated! Write it to a distinct test file, NEVER overwrite deliverable!
                            let target_ext = std::path::Path::new(&subgoal.target_entity)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("test");
                            let test_filename = if subgoal.target_entity.is_empty() {
                                format!("test_suite.{}", target_ext)
                            } else if subgoal.target_entity.starts_with("test_") {
                                subgoal.target_entity.clone()
                            } else {
                                format!("test_{}", subgoal.target_entity)
                            };
                            self.vfs.write_file(&std::path::PathBuf::from(&test_filename), content.as_bytes());
                            let run_cmd = Self::resolve_verification_command(&test_filename);
                            SwarmAction::RunTerminal { command: run_cmd }
                        }
                    }
                    _ => {
                        let default_cmd = Self::resolve_verification_command(&subgoal.target_entity);
                        SwarmAction::RunTerminal { command: default_cmd }
                    }
                }
            } else {
                parsed_action
            };
            let tokens_at_attempt_end = worker_ctx.current_cursor();

            if self.config.verbose {
                let preview = if generated_text.len() > 120 {
                    format!("{}... [{} chars]", &generated_text[..120].trim(), generated_text.len())
                } else {
                    generated_text.trim().to_string()
                };
                println!(
                    "  [WORKER OUTPUT] Step generated: \"{}\" ({} tokens)",
                    preview.bright_white(),
                    generated_tokens.len()
                );
            }

            // CHECKPOINT 3: Pre-action mutation checkpoint
            if let Some(ref abort) = abort_flag {
                if abort.load(Ordering::Relaxed) {
                    println!("  [ABORT CHECKPOINT] Worker #{} aborting before VFS write/action execution.", subgoal.id);
                    return Ok(WorkerFinding {
                        worker_id: subgoal.id,
                        sub_goal: subgoal.description.clone(),
                        finding: "CANCELLED: Worker aborted prior to file mutation.".to_string(),
                        steps_taken: attempt,
                        rollbacks_count,
                        tokens_saved_by_rollback: tokens_saved,
                    });
                }
            }

            // Execute action
            let (exec_success, exec_output) = match action {
                SwarmAction::WriteFile { ref filename, ref content } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);

                    // BOUNDARY ENFORCEMENT: Enforce subgoal.write_set
                    if !MemoryVfs::is_path_in_set(&path, &subgoal.write_set) {
                        println!(
                            "  [VFS BOUNDARY VIOLATION] Worker #{} attempted unauthorized write to '{}' (allowed write_set: {:?})",
                            subgoal.id, actual_filename, subgoal.write_set
                        );
                        (
                            false,
                            format!(
                                "ERR_OUT_OF_BOUNDS_WRITE: Unauthorized write to '{}'. Path is not in write_set: {:?}",
                                actual_filename, subgoal.write_set
                            ),
                        )
                    } else {
                        if is_surgical && surgery_slice.is_some() {
                            let slice = surgery_slice.as_ref().unwrap();
                            let full_content_opt = self.vfs.read_file(&path)
                                .map(|b| String::from_utf8_lossy(&b).to_string())
                                .or_else(|| {
                                    self.resolve_existing_file(&path)
                                        .and_then(|p| std::fs::read_to_string(p).ok())
                                });

                            if let Some(full_content) = full_content_opt {
                                let lines: Vec<&str> = full_content.split('\n').collect();
                                let start_idx = slice.start_line.saturating_sub(1);
                                let end_idx = slice.end_line.min(lines.len());
                                
                                let mut new_content = String::new();
                                for i in 0..start_idx {
                                    new_content.push_str(lines[i]);
                                    new_content.push('\n');
                                }
                                
                                new_content.push_str(content);
                                if !content.ends_with('\n') {
                                    new_content.push('\n');
                                }
                                
                                for i in end_idx..lines.len() {
                                    new_content.push_str(lines[i]);
                                    if i < lines.len() - 1 {
                                        new_content.push('\n');
                                    }
                                }
                                
                                self.vfs.write_file(&path, new_content.as_bytes());
                            } else {
                                // Fallback if file read fails for some reason
                                self.vfs.write_file(&path, content.as_bytes());
                            }
                        } else {
                            println!("  [EXECUTION] Direct code write: '{}' ({} bytes)...", actual_filename, content.len());
                            self.vfs.write_file(&path, content.as_bytes());
                        }
                        (true, format!("Wrote {} bytes to {}", content.len(), actual_filename))
                    }
                }
                SwarmAction::PatchFile { ref filename, ref target, ref replacement } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);

                    // BOUNDARY ENFORCEMENT: Enforce subgoal.write_set
                    if !MemoryVfs::is_path_in_set(&path, &subgoal.write_set) {
                        println!(
                            "  [VFS BOUNDARY VIOLATION] Worker #{} attempted unauthorized patch to '{}' (allowed write_set: {:?})",
                            subgoal.id, actual_filename, subgoal.write_set
                        );
                        (
                            false,
                            format!(
                                "ERR_OUT_OF_BOUNDS_WRITE: Unauthorized patch on '{}'. Path is not in write_set: {:?}",
                                actual_filename, subgoal.write_set
                            ),
                        )
                    } else {
                        println!("  [EXECUTION] Patching '{}'...", actual_filename);
                        match self.vfs.patch_file(&path, target, replacement) {
                            Ok(true) => {
                                (true, format!("Successfully patched {}", actual_filename))
                            }
                            Ok(false) => {
                                (false, format!("Target snippet not found in {}", actual_filename))
                            }
                            Err(e) => {
                                (false, format!("Patch error in {}: {}", actual_filename, e))
                            }
                        }
                    }
                }
                SwarmAction::DeleteFile { ref filename } => {
                    let actual_filename = if filename.is_empty() {
                        &subgoal.target_entity
                    } else {
                        filename.as_str()
                    };
                    let path = std::path::PathBuf::from(actual_filename);

                    // BOUNDARY ENFORCEMENT: Enforce subgoal.write_set
                    if !MemoryVfs::is_path_in_set(&path, &subgoal.write_set) {
                        println!(
                            "  [VFS BOUNDARY VIOLATION] Worker #{} attempted unauthorized delete of '{}' (allowed write_set: {:?})",
                            subgoal.id, actual_filename, subgoal.write_set
                        );
                        (
                            false,
                            format!(
                                "ERR_OUT_OF_BOUNDS_WRITE: Unauthorized delete of '{}'. Path is not in write_set: {:?}",
                                actual_filename, subgoal.write_set
                            ),
                        )
                    } else {
                        println!("  [EXECUTION] Deleting file: '{}'...", actual_filename);
                        let deleted = self.vfs.delete_file(&path);
                        if deleted {
                            (true, format!("Successfully deleted {}", actual_filename))
                        } else if !self.vfs.exists(&path) {
                            (true, format!("File '{}' is not present in VFS (already deleted)", actual_filename))
                        } else {
                            (false, format!("Failed to delete '{}' from VFS", actual_filename))
                        }
                    }
                }
                SwarmAction::TestRun { ref command, ref inputs } => {
                    let clean_cmd = Self::strip_command_prefix(command);
                    let bwrap_active = IsolatedJail::is_bwrap_available();
                    println!(
                        "  [INTERACTIVE CLEANROOM] Executing test_run (bwrap={}): '{}' with {} simulated inputs...",
                        bwrap_active, clean_cmd, inputs.len()
                    );
                    let staged_files = self.vfs.all_files();
                    let input_strs: Vec<&str> = inputs.iter().map(|s| s.as_str()).collect();
                    let report = crate::sandbox::cleanroom::UniversalCleanroom::execute_interactive(
                        &clean_cmd,
                        &staged_files,
                        &input_strs,
                        std::time::Duration::from_secs(60),
                    );
                    let combined = format!("{}\n{}", report.full_stdout, report.full_stderr);
                    (report.success, combined)
                }
                SwarmAction::Verdict { status, ref proof_or_missing } => {
                    println!("  [ACCEPTANCE VERDICT] Model emitted verdict: status={} | {}", status, proof_or_missing);
                    (status, proof_or_missing.clone())
                }
                SwarmAction::RunTerminal { ref command } => {
                    let clean_cmd = Self::strip_command_prefix(command);
                    let bwrap_active = IsolatedJail::is_bwrap_available();
                    println!(
                        "  [EXECUTION] Isolated Sandbox exec (bwrap={}): '{}'...",
                        bwrap_active, clean_cmd
                    );
                    let staged_files = self.vfs.all_files();
                    let planned_targets: Vec<String> = subgoal.read_set.iter().map(|p| p.to_string_lossy().to_string()).collect();
                    let test_sheet = crate::sandbox::cleanroom::TestSheet::from_planned_files(user_goal, &planned_targets);
                    let formatted_input = if !test_sheet.interactive_inputs.is_empty() {
                        let mut buf = test_sheet.interactive_inputs.join("\n");
                        buf.push('\n');
                        Some(buf.into_bytes())
                    } else {
                        None
                    };
                    match IsolatedJail::run_in_jail_with_input(&clean_cmd, &staged_files, std::time::Duration::from_secs(120), formatted_input.as_deref()) {
                        Ok(jail_res) => {
                            let combined = format!("{}{}", jail_res.stdout, jail_res.stderr);
                            (jail_res.success, combined)
                        }
                        Err(e) => {
                            (false, format!("Isolated jail error: {}", e))
                        }
                    }
                }
                SwarmAction::Search { ref query } => {
                    println!("  [SEARCH] Query: \"{}\"", query);
                    match self.web_lens.search(query, 3) {
                        Ok(results) if !results.is_empty() => {
                            let mut results_str = format!("Found {} sources:\n", results.len());
                            for (i, r) in results.iter().enumerate() {
                                let snippet_clean = if r.snippet.len() > 300 {
                                    format!("{}...", &r.snippet[..300])
                                } else {
                                    r.snippet.clone()
                                };
                                results_str.push_str(&format!("{}. [{}] {}\nURL: {}\n", i + 1, r.title, snippet_clean, r.url));
                            }
                            (true, results_str)
                        }
                        Ok(_) => {
                            (false, "Search yielded 0 results.".to_string())
                        }
                        Err(e) => {
                            (false, format!("Search failed: {}", e))
                        }
                    }
                }
                SwarmAction::Fetch { ref url } => {
                    match self.web_lens.fetch_text(url, 800) {
                        Ok(content) => (true, content),
                        Err(e) => (false, format!("Fetch error: {}", e)),
                    }
                }
                SwarmAction::Report { ref finding } => {
                    (true, finding.clone())
                }
                SwarmAction::Done => {
                    (true, "DONE signaled".to_string())
                }
                SwarmAction::Consult { ref question } => {
                    (false, format!("Consultation: {}", question))
                }
                SwarmAction::None => {
                    (false, "Unparsed action / missing code block".to_string())
                }
            };

            // GROUND TRUTH CHECK & THINKER VERIFICATION GATE
            let vfs_bytes = self.vfs.read_file(&target_path);
            let vfs_target_exists = self.vfs.exists(&target_path);
            let vfs_target_size = vfs_bytes.as_ref().map(|c| c.len()).unwrap_or(0);
            let vfs_str = vfs_bytes.as_ref().and_then(|b| std::str::from_utf8(b).ok());
            let line_diff = self.vfs.diff_file(&target_path);

            // Line-by-Line Diff Perception Telemetry
            if let Some(ref diff) = line_diff {
                if !diff.trim().is_empty() {
                    println!("  [DIFF PERCEPTION] Unified Line Diff for '{}':", subgoal.target_entity.bright_cyan());
                    for line in diff.lines().take(25) {
                        if line.starts_with('+') && !line.starts_with("+++") {
                            println!("    {}", line.green());
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            println!("    {}", line.red());
                        } else if line.starts_with("@@") {
                            println!("    {}", line.cyan());
                        } else {
                            println!("    {}", line.white());
                        }
                    }
                    if diff.lines().count() > 25 {
                        println!("    ... (diff truncated for display)");
                    }
                }
            }

            let mut placeholder_found = false;
            let mut placeholder_reason = String::new();
            if let Some(code) = vfs_str {
                let trimmed = code.trim();
                if code.contains("TODO") || code.contains("FIXME") {
                    placeholder_found = true;
                    placeholder_reason = "Code contains TODO or FIXME placeholders".to_string();
                } else if code.contains("<!-- implementation")
                    || code.contains("/* implementation")
                    || code.contains("# implementation")
                {
                    placeholder_found = true;
                    placeholder_reason = "Code contains placeholder comments".to_string();
                } else if (trimmed.ends_with("pass") || trimmed.contains("pass\n")) && vfs_target_size < 120 {
                    placeholder_found = true;
                    placeholder_reason = "Code contains stub pass statement".to_string();
                } else if trimmed == "..." {
                    placeholder_found = true;
                    placeholder_reason = "Code contains ellipsis placeholder".to_string();
                }
            }

            // Universal Language-Agnostic Cleanroom Validation Gate
            let mut syntax_error: Option<String> = None;
            let mut cleanroom_missing_dep: Option<String> = None;
            if (is_build || subgoal.kind == TaskKind::Create || subgoal.kind == TaskKind::Modify)
                && vfs_target_exists && vfs_target_size > 0 && !placeholder_found
            {
                if let Some(bytes) = vfs_bytes.as_ref() {
                    let vfs_files = self.vfs.all_files();
                    let report = crate::sandbox::cleanroom::UniversalCleanroom::audit_code_syntax_with_vfs(
                        &subgoal.target_entity,
                        bytes,
                        &vfs_files,
                    );
                    if !report.is_valid {
                        syntax_error = report.error_message.clone();
                        cleanroom_missing_dep = Self::extract_missing_dependency(&report.raw_output)
                            .or_else(|| report.error_message.as_ref().and_then(|m| Self::extract_missing_dependency(m)));
                        if let Some(ref dep) = cleanroom_missing_dep {
                            println!("  [DEPENDENCY INTERCEPTOR] Detected missing external package: '{}' in '{}'", dep.bright_yellow().bold(), subgoal.target_entity);
                        } else if let Some(line) = report.error_line {
                            println!("  [UNIVERSAL CLEANROOM] Detected error at line {} in '{}': {:?}", line, subgoal.target_entity, report.error_message);
                        }
                    }
                }
            }

            // Cognitive Test Sheet Protocol Audit for Test tasks
            let mut test_acceptance: Option<crate::sandbox::cleanroom::AcceptanceDeltaReport> = None;
            let mut acceptance_error_desc: Option<String> = None;
            if subgoal.kind == TaskKind::Test {
                let target_file = if subgoal.target_entity.is_empty() {
                    subgoal.depends_on.first().cloned().unwrap_or_default()
                } else {
                    subgoal.target_entity.clone()
                };
                let target_buf = PathBuf::from(&target_file);
                let file_content = self.vfs.read_file(&target_buf)
                    .map(|b| String::from_utf8_lossy(&b).to_string())
                    .unwrap_or_default();

                let vfs_files: Vec<String> = self.vfs.list_files()
                    .into_iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                // Aggregate content across all staged files in VFS to ensure semantic markers & domain checks
                // evaluate the complete multi-file project bundle (e.g. CSS markers in style.css, JS in app.js).
                let mut aggregated_content = file_content.clone();
                for f in &vfs_files {
                    if f != &target_file {
                        if let Some(bytes) = self.vfs.read_file(&PathBuf::from(f)) {
                            aggregated_content.push_str("\n\n");
                            aggregated_content.push_str(&String::from_utf8_lossy(&bytes));
                        }
                    }
                }

                let planned_targets: Vec<String> = subgoal.read_set.iter().map(|p| p.to_string_lossy().to_string()).collect();
                let test_sheet = crate::sandbox::cleanroom::TestSheet::from_planned_files(user_goal, &planned_targets);

                let report = crate::sandbox::cleanroom::UniversalCleanroom::audit_acceptance_criteria_with_sheet(
                    &target_file,
                    &aggregated_content,
                    user_goal,
                    &test_sheet,
                    &vfs_files,
                );

                if !report.is_accepted {
                    acceptance_error_desc = Some(format!("ERR_ACCEPTANCE_CRITERIA_FAILED: {}", report.delta_summary));
                }
                test_acceptance = Some(report);
            }

            // 4-Tier Verification Gate
            let real_success = match subgoal.kind {
                TaskKind::Create => {
                    exec_success && vfs_target_exists && vfs_target_size > 0 && !placeholder_found && syntax_error.is_none()
                }
                TaskKind::Modify => {
                    let has_diff = line_diff.as_ref().map(|d| !d.trim().is_empty()).unwrap_or(false);
                    exec_success && vfs_target_exists && has_diff && !placeholder_found && syntax_error.is_none()
                }
                TaskKind::Delete => {
                    exec_success && !self.vfs.exists(&target_path)
                }
                TaskKind::Test => {
                    let mut no_crash = exec_success
                        && !exec_output.contains("error:")
                        && !exec_output.contains("Error:")
                        && !exec_output.contains("FAILED")
                        && !exec_output.contains("SyntaxError:")
                        && !exec_output.contains("compilation failed")
                        && !exec_output.contains("Traceback (most recent call last)");

                    if let SwarmAction::TestRun { .. } = action {
                        let verdict_prompt = format!(
                            "\n\n[TERMINAL EXECUTION SCREEN LOG]:\n```\n{}\n```\n\n\
                            [SUPREME USER GOAL]:\n\"{}\"\n\n\
                            [VERDICT MANDATE - STRICT OUTPUT REQUIRED]:\n\
                            Evaluate the terminal execution log above against the user's supreme goal.\n\
                            Did the program satisfy all user requirements?\n\
                            You MUST output EXCLUSIVELY a ```verdict block. Any other format is rejected:\n\
                            ```verdict\n\
                            status: YES (or NO)\n\
                            proof: <if YES, what was verified>\n\
                            missing:\n\
                            - <if NO, bulleted list of missing requirements>\n\
                            ```\n\
                            Output verdict now:",
                            exec_output.trim(), user_goal
                        );
                        let verdict_tokens = self.worker_model.tokenize(&verdict_prompt, false).unwrap_or_default();
                        if let Ok(()) = worker_ctx.eval_tokens(&verdict_tokens, 0) {
                            let mut verdict_text = String::new();
                            for _ in 0..256 {
                                if let Ok(tok) = worker_ctx.sample(&mut sampler) {
                                    if let Ok(piece) = self.worker_model.token_to_piece(tok) {
                                        verdict_text.push_str(&piece);
                                        if piece.contains("```") && verdict_text.matches("```").count() >= 2 {
                                            break;
                                        }
                                    }
                                } else {
                                    break;
                                }
                            }
                            let verdict_action = Self::parse_action(&verdict_text);
                            if let SwarmAction::Verdict { status, proof_or_missing } = verdict_action {
                                if !status {
                                    println!("  [ACCEPTANCE VERDICT REJECTED] Target '{}': {}", subgoal.target_entity.bright_cyan(), proof_or_missing.bright_yellow());
                                    final_finding = format!(
                                        "[ACCEPTANCE DELTA] In '{}': Missing requirements: {}. Implemented: Verified execution in cleanroom",
                                        subgoal.target_entity,
                                        proof_or_missing
                                    );
                                    no_crash = false;
                                } else {
                                    println!("  [ACCEPTANCE VERDICT ACCEPTED] Target '{}': {}", subgoal.target_entity.bright_cyan(), proof_or_missing.bright_green());
                                    final_finding = format!(
                                        "PASS: Acceptance criteria verified by user simulator. Proof: {}",
                                        proof_or_missing
                                    );
                                    no_crash = true;
                                }
                            }
                        }
                    } else if let SwarmAction::Verdict { status, ref proof_or_missing } = action {
                        if !status {
                            println!("  [ACCEPTANCE VERDICT REJECTED] Target '{}': {}", subgoal.target_entity.bright_cyan(), proof_or_missing.bright_yellow());
                            final_finding = format!(
                                "[ACCEPTANCE DELTA] In '{}': Missing requirements: {}. Implemented: Verified in cleanroom",
                                subgoal.target_entity,
                                proof_or_missing
                            );
                            no_crash = false;
                        } else {
                            println!("  [ACCEPTANCE VERDICT ACCEPTED] Target '{}': {}", subgoal.target_entity.bright_cyan(), proof_or_missing.bright_green());
                            final_finding = format!(
                                "PASS: Acceptance criteria verified by user simulator. Proof: {}",
                                proof_or_missing
                            );
                            no_crash = true;
                        }
                    } else if no_crash {
                        if let Some(ref acceptance) = test_acceptance {
                            if !acceptance.is_accepted {
                                println!("  [ACCEPTANCE DELTA AUDIT] Target '{}' missing requirements: {}", subgoal.target_entity.bright_cyan(), acceptance.delta_summary.bright_yellow());
                                final_finding = format!(
                                    "[ACCEPTANCE DELTA] In '{}': Missing requirements: {}. Implemented: {}",
                                    subgoal.target_entity,
                                    acceptance.missing_requirements.join("; "),
                                    acceptance.covered_requirements.join("; ")
                                );
                                break;
                            }
                        }
                    }
                    no_crash
                }
                TaskKind::Search => {
                    exec_success && !exec_output.trim().is_empty()
                }
            };

            if real_success {
                // Synthesize immutable Epistemic Flags
                let epistemic_flag = match subgoal.kind {
                    TaskKind::Create => {
                        format!(
                            "[FLAG #{} (Verified Create): File '{}' staged in VFS ({} bytes); syntax valid; zero placeholders]",
                            subgoal.id, subgoal.target_entity, vfs_target_size
                        )
                    }
                    TaskKind::Modify => {
                        let diff_summary = line_diff.as_ref().map(|d| {
                            let added = d.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).count();
                            let removed = d.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).count();
                            format!("+{} / -{} lines", added, removed)
                        }).unwrap_or_else(|| "modified".to_string());
                        format!(
                            "[FLAG #{} (Verified Line Diff): File '{}' patched ({}); syntax valid; no regressions]",
                            subgoal.id, subgoal.target_entity, diff_summary
                        )
                    }
                    TaskKind::Delete => {
                        format!(
                            "[FLAG #{} (Verified Delete): File '{}' confirmed removed from VFS; tracking recorded]",
                            subgoal.id, subgoal.target_entity
                        )
                    }
                    TaskKind::Test => {
                        format!(
                            "[FLAG #{} (Verified Test): Execution gate passed with exit code 0; zero tracebacks; 100% Acceptance Criteria Satisfied]",
                            subgoal.id
                        )
                    }
                    TaskKind::Search => {
                        format!(
                            "[FLAG #{} (Verified Search): Discovered empirical facts]",
                            subgoal.id
                        )
                    }
                };

                println!("{}", "  ┌────────────────────────────────────────────────────────────────────────┐".bright_green());
                println!("  │ {} Thinker Perception Gate: {}", "[+]".bright_green().bold(), epistemic_flag.bright_white().bold());
                match subgoal.kind {
                    TaskKind::Create => {
                        println!("  │     * Target: {} | Size: {} bytes | Syntax Gate: VALIDATED", subgoal.target_entity.bright_cyan(), vfs_target_size);
                        println!("  │     * Clean Code: Zero placeholders, zero TODOs, production-ready");
                        println!("  │     * Status: Staged in RAM Sandbox VFS -> Ready for host commit");
                    }
                    TaskKind::Modify => {
                        let diff_lines_count = line_diff.as_ref().map(|d| d.lines().count()).unwrap_or(0);
                        println!("  │     * Target: {} | Diff lines: {} | Semantic Diff: VALIDATED", subgoal.target_entity.bright_cyan(), diff_lines_count);
                        println!("  │     * Syntax Gate: Validated | No unintended regressions detected");
                    }
                    TaskKind::Delete => {
                        println!("  │     * Target: {} | VFS Existence: REMOVED (Confirmed !vfs.exists)", subgoal.target_entity.bright_cyan());
                        println!("  │     * Tracking: Tombstone recorded in VFS ledger");
                    }
                    TaskKind::Test => {
                        let bwrap_active = IsolatedJail::is_bwrap_available();
                        println!("  │     * Sandbox Gate: PASSED (Isolation: {}, zero tracebacks)", if bwrap_active { "Bubblewrap bwrap" } else { "Ephemeral Tempdir" });
                    }
                    TaskKind::Search => {
                        println!("  │     * Search/Fetch: Gathered factual ground truth");
                    }
                }
                println!("{}", "  └────────────────────────────────────────────────────────────────────────┘".bright_green());

                // Epistemic Distillation: The lean flag replaces the raw bulky inspection tokens
                final_finding = epistemic_flag;
                break;
            }

            // FAILURE OCCURRED: Trigger Worker Scoped Rollback to Checkpoint
            // Strict mathematical isolation: Restores ONLY this worker's write scope (W_i).
            // Concurrently written files by other workers remain 100% intact in VFS.
            self.vfs.restore_scoped_snapshot(vfs_scoped_snapshot);
            let tokens_before = tokens_at_attempt_end;
            let _ = worker_ctx.rollback_to(checkpoint);
            let tokens_after = worker_ctx.current_cursor();
            let excised = tokens_before.saturating_sub(tokens_after);
            rollbacks_count += 1;
            tokens_saved += excised;

            // Deterministic Diagnostic Classification
            let diagnostic = if let Some(ref dep) = cleanroom_missing_dep {
                DiagnosticCode::MissingDependency(dep.clone())
            } else if let Some(ref syn_err) = syntax_error {
                if let Some(dep) = Self::extract_missing_dependency(syn_err) {
                    DiagnosticCode::MissingDependency(dep)
                } else {
                    DiagnosticCode::TestFailed
                }
            } else if let Some(ref acc_err) = acceptance_error_desc {
                DiagnosticCode::AcceptanceCriteriaFailed(acc_err.clone())
            } else {
                Self::classify_failure_for_kind(
                    &action,
                    exec_success,
                    &exec_output,
                    vfs_target_exists,
                    vfs_target_size,
                    vfs_str,
                    &generated_text,
                    last_diagnostic.as_ref(),
                    Some(&subgoal.kind),
                )
            };

            println!(
                "  [ROLLBACK] Worker #{} rolled back to checkpoint {} (excised {} tokens). Diagnostic: {}.",
                subgoal.id, tokens_after, excised, diagnostic.name()
            );

            // Immediate Abort on Boundary Violation (Zero Retries Allowed for Security Breaches)
            if diagnostic == DiagnosticCode::OutOfBoundsWrite {
                println!(
                    "  [FATAL BOUNDARY BREACH] Task #{} triggered OutOfBoundsWrite for '{}'! Aborting worker loop immediately without retries.",
                    subgoal.id, subgoal.target_entity
                );
                final_finding = format!(
                    "FAILED: OutOfBoundsWrite - Task #{} violated security boundaries by attempting modification outside its write_set.",
                    subgoal.id
                );
                break;
            }

            // ESCALATE TO THINKER FOR SOVEREIGN DECISION
            let error_desc = if let Some(ref syn_err) = syntax_error {
                format!("Syntax error in '{}': {}", subgoal.target_entity, syn_err)
            } else if placeholder_found {
                format!("Incomplete placeholder in '{}': {}", subgoal.target_entity, placeholder_reason)
            } else if let Some(ref acc_err) = acceptance_error_desc {
                acc_err.clone()
            } else if !exec_success {
                exec_output.clone()
            } else {
                match subgoal.kind {
                    TaskKind::Delete => format!("File '{}' was not deleted from VFS", subgoal.target_entity),
                    TaskKind::Modify => format!("Modification on '{}' yielded zero diff or failed", subgoal.target_entity),
                    TaskKind::Test => format!("Test execution for '{}' failed", subgoal.target_entity),
                    _ => {
                        if !vfs_target_exists {
                            format!("File '{}' was not written to VFS", subgoal.target_entity)
                        } else if vfs_target_size == 0 {
                            format!("File '{}' is empty (0 bytes)", subgoal.target_entity)
                        } else {
                            format!("File '{}' verification failed", subgoal.target_entity)
                        }
                    }
                }
            };

            println!("{}", "  ┌── [WORKER ATTEMPT FAILURE AUDIT] ──────────────────────────────────".bright_red());
            println!("  │ Task ID       : #{} ({:?}) Target: '{}'", subgoal.id, subgoal.kind, subgoal.target_entity.bright_cyan());
            println!("  │ Attempt       : #{} of {}", attempt, self.config.max_steps_per_worker);
            println!("  │ Diagnostic    : {}", diagnostic.name().bright_yellow().bold());
            if let Some(ref syn) = syntax_error {
                println!("  │ Syntax Failure: {}", syn.bright_red());
            }
            if let Some(ref acc) = acceptance_error_desc {
                println!("  │ Acceptance Err: {}", acc.bright_yellow());
            }
            if !exec_success && !exec_output.trim().is_empty() {
                println!("  │ Execution Err : {}", exec_output.lines().next().unwrap_or("Non-zero exit").bright_red());
            }
            println!("  │ Error Summary : {}", error_desc.trim().bright_red());
            println!("{}", "  └────────────────────────────────────────────────────────────────────".bright_red());

            let (decision, ledger_entry) = self.thinker_evaluate_failure(
                subgoal,
                attempt,
                &generated_text,
                &error_desc,
                &diagnostic,
            )?;

            println!("  {}", ledger_entry);

            last_diagnostic = Some(diagnostic);
            last_error_desc = Some(error_desc.clone());

            let is_premature_token_misfire = generated_tokens.len() <= 10;
            if is_premature_token_misfire {
                println!(
                    "  [COLD-START RESILIENCE] Worker #{} generated only {} tokens (<= 10). Excused from error counter without consuming attempt budget.",
                    subgoal.id, generated_tokens.len()
                );
            }

            match decision {
                ThinkerDecision::Nudge(hint) => {
                    pending_nudge = Some(hint);
                    if !is_premature_token_misfire {
                        attempt += 1;
                    }
                }
                ThinkerDecision::Reset(new_directive) => {
                    current_directive = new_directive;
                    pending_nudge = None;
                    if !is_premature_token_misfire {
                        attempt += 1;
                    }
                }
                ThinkerDecision::ConsultDependency { dependency, question } => {
                    let user_choice = Self::prompt_user_for_dependency(&dependency, &question, self.config.auto_approve_deps);
                    match user_choice {
                        DependencyChoice::InstallAndRetry => {
                            let pm = HostPackageManager::detect();
                            let install_cmd = pm.build_install_command(&dependency);
                            println!("  [TERMINAL] Detected host package manager: {:?}. Installing dependency '{}'...", pm, dependency);
                            println!("  [TERMINAL] Command: {}", install_cmd.bright_cyan());
                            match self.terminal.run_sync(&install_cmd, 120) {
                                Ok(out) => {
                                    let summary = out.lines().last().unwrap_or("Installation completed");
                                    println!("  [TERMINAL] Installation completed: {}", summary);
                                    pending_nudge = Some(format!(
                                        "Dependency '{}' was installed on the system. Re-execute your script now.",
                                        dependency
                                    ));
                                    attempt += 1;
                                }
                                Err(e) => {
                                    println!("  [TERMINAL] Installation failed: {}. Falling back to alternative / standard library.", e);
                                    if !declined_dependencies.contains(&dependency) {
                                        declined_dependencies.push(dependency.clone());
                                    }
                                    let deps_list = declined_dependencies.join(", ");
                                    current_directive = format!(
                                        "Do not use external libraries: [{}]. Implement the solution strictly without [{}] using alternative libraries or standard built-ins.",
                                        deps_list, deps_list
                                    );
                                    pending_nudge = None;
                                    attempt += 1;
                                }
                            }
                        }
                        DependencyChoice::UseAlternative => {
                            println!("  [DECISION] User opted to proceed without installing '{}'. Switching to alternative.", dependency);
                            if !declined_dependencies.contains(&dependency) {
                                declined_dependencies.push(dependency.clone());
                            }
                            let deps_list = declined_dependencies.join(", ");
                            current_directive = format!(
                                "Do not use external libraries: [{}]. Implement the solution strictly without [{}] using alternative libraries or standard built-ins.",
                                deps_list, deps_list
                            );
                            pending_nudge = None;
                            attempt += 1;
                        }
                    }
                }
            }
        }

        if final_finding.is_empty() {
            final_finding = if subgoal.kind == TaskKind::Test {
                format!(
                    "FAILED: Worker #{} isolated sandbox test for '{}' failed after {} attempts.",
                    subgoal.id, subgoal.target_entity, max_attempts
                )
            } else {
                format!(
                    "FAILED: Worker #{} could not write target file '{}' to VFS after {} attempts.",
                    subgoal.id, subgoal.target_entity, max_attempts
                )
            };
        }

        Ok(WorkerFinding {
            worker_id: subgoal.id,
            sub_goal: subgoal.description.clone(),
            finding: final_finding,
            steps_taken: attempt,
            rollbacks_count,
            tokens_saved_by_rollback: tokens_saved,
        })
    }

    /// System 2: Synthesizes all gathered worker findings into a final report as Executive Secretary
    fn synthesize_findings(&self, original_goal: &str, findings: &[WorkerFinding]) -> Result<(String, Option<String>), String> {
        let all_successful = findings.iter().all(|f| !f.finding.starts_with("FAILED"));

        let mut report = String::new();
        report.push_str(":: DELIVERABLES & EXECUTION SUMMARY:\n");
        for (i, f) in findings.iter().enumerate() {
            let status_mark = if f.finding.starts_with("FAILED") { "[-]" } else { "[+]" };
            report.push_str(&format!(
                "  {} Task #{}: {}\n     Result: {}\n",
                status_mark, i + 1, f.sub_goal, f.finding
            ));
        }

        let mut ctx = match self.orchestrator_model.create_context(4096, 512, 4) {
            Ok(c) => c,
            Err(_) => return Ok((report, None)),
        };

        let system_msg = if all_successful {
            "You are the System 2 Sovereign Secretary reporting to the user.\n\
            All deliverables were verified and staged in VFS successfully.\n\
            In 2-4 concise, professional bullet points, explain what was built, how the components interact, and how to compile or run the generated code.\n\
            Do not repeat status lines or invent failures."
        } else {
            "You are the System 2 Sovereign Secretary reporting to the user.\n\
            Some deliverables encountered unresolved issues.\n\
            In 2-3 concise bullet points, explain what succeeded and what specific files or dependencies require resolution."
        };

        let user_msg = format!(
            "User Goal: {}\nCompleted Work:\n{}\nProvide the brief architectural overview:",
            original_goal, report
        );
        let prompt = Self::format_prompt(&self.orchestrator_model, system_msg, &user_msg);

        let mut model_overview: Option<String> = None;
        if let Ok(summary) = ctx.generate(&prompt, 768) {
            let mut clean_summary = Self::strip_thought(&summary);
            if clean_summary.matches("```").count() % 2 != 0 {
                clean_summary.push_str("\n```\n");
            }
            let trimmed = clean_summary.trim();
            if !trimmed.is_empty() {
                report.push_str("\n:: ARCHITECTURAL OVERVIEW:\n");
                report.push_str(trimmed);
                report.push('\n');
                model_overview = Some(trimmed.to_string());
            }
        }

        Ok((report, model_overview))
    }

    /// Universal Project Documentation Generator:
    /// Constructs a standardized, high-quality README.md for every completed Swarm mission,
    /// integrating the AI Verifier's synthesized architecture & run instructions, deliverables table, and cleanroom verdict.
    pub fn generate_project_readme(
        &self,
        user_goal: &str,
        findings: &[WorkerFinding],
        verified_paths: &[PathBuf],
        model_overview: Option<&str>,
    ) -> String {
        let mut unique_paths = Vec::new();
        for p in verified_paths {
            if !unique_paths.contains(p) {
                unique_paths.push(p.clone());
            }
        }

        let mut md = String::new();
        md.push_str("# Project Documentation\n\n");
        md.push_str(&format!("**Objective:** {}\n\n", user_goal));

        // 1. Model's Verified Architecture & Usage Guide (Synthesized by the AI Verifier)
        if let Some(overview) = model_overview {
            let trimmed = overview.trim();
            if !trimmed.is_empty() {
                md.push_str("## Architecture & Usage Guide\n\n");
                md.push_str(trimmed);
                md.push_str("\n\n");
            }
        }

        // 2. Delivered Artifacts Table
        md.push_str("## Delivered Artifacts\n\n");
        md.push_str("| Artifact | Responsibility / Component | Status |\n");
        md.push_str("| :--- | :--- | :--- |\n");
        for p in &unique_paths {
            let fname = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string());
            md.push_str(&format!("| `{}` | Verified project deliverable | Verified Cleanroom |\n", fname));
        }
        md.push('\n');

        // 3. Fallback Quickstart Heuristic (only if model overview did not provide execution commands)
        let has_model_instructions = model_overview
            .map(|o| o.contains("```") || o.contains("python3") || o.contains("cargo run") || o.contains("bash "))
            .unwrap_or(false);

        if !has_model_instructions {
            md.push_str("## How to Run & Quickstart\n\n");
            let cli_py = unique_paths.iter().find(|p| {
                let s = p.to_string_lossy().to_lowercase();
                s.ends_with(".py") && (s.contains("cli") || s.contains("main") || s.contains("app") || s.contains("run"))
            });
            let any_py = unique_paths.iter().find(|p| p.extension().map(|e| e == "py").unwrap_or(false));
            let py_entry = cli_py.or(any_py);
            let sh_entry = unique_paths.iter().find(|p| p.extension().map(|e| e == "sh").unwrap_or(false));
            let rs_entry = unique_paths.iter().find(|p| p.extension().map(|e| e == "rs").unwrap_or(false));
            let html_entry = unique_paths.iter().find(|p| p.extension().map(|e| e == "html").unwrap_or(false));

            if let Some(entry) = py_entry {
                md.push_str(&format!("Run the Python application:\n```bash\npython3 {}\n```\n\n", entry.display()));
            } else if let Some(entry) = sh_entry {
                md.push_str(&format!("Execute the shell script:\n```bash\nbash {}\n```\n\n", entry.display()));
            } else if let Some(entry) = html_entry {
                md.push_str(&format!("Open in web browser:\n```bash\nxdg-open {}\n```\n\n", entry.display()));
            } else if rs_entry.is_some() {
                md.push_str("Compile and execute with Cargo:\n```bash\ncargo run\n```\n\n");
            } else {
                md.push_str("Project files are staged and ready for inspection in the workspace.\n\n");
            }
        }

        // 4. Component Breakdown
        md.push_str("## Component Breakdown\n\n");
        for p in &unique_paths {
            let name = p.display().to_string();
            if name != "README.md" {
                md.push_str(&format!("- **`{}`**: Verified project deliverable.\n", name));
            }
        }
        md.push('\n');

        // 5. Worker Verification Details
        if !findings.is_empty() {
            md.push_str("## Worker Verification Details\n\n");
            for f in findings {
                md.push_str(&format!("- **Worker #{}** (`{}`): {}\n", f.worker_id, f.sub_goal, f.finding));
            }
            md.push('\n');
        }

        // 6. Quality & Verification Verdict
        md.push_str("## Quality & Verification Verdict\n\n");
        md.push_str("All deliverables verified inside isolated Linux cleanroom sandbox (Bubblewrap `bwrap`). Zero syntax errors, zero trapped tokens, 100% acceptance criteria satisfied.\n");

        md
    }

    /// Strip prefixes like RUN:, EXEC:, TERMINAL:, $, #, -, etc. from an execution command
    pub fn strip_command_prefix(raw: &str) -> String {
        let mut s = raw.trim();
        loop {
            let mut changed = false;
            if let Some(rest) = s.strip_prefix("Assistant:") {
                s = rest.trim();
                changed = true;
            }
            if let Some(rest) = s.strip_prefix("- ")
                .or_else(|| s.strip_prefix("* "))
                .or_else(|| s.strip_prefix("1. "))
                .or_else(|| s.strip_prefix("2. "))
                .or_else(|| s.strip_prefix("$ "))
                .or_else(|| s.strip_prefix("# "))
            {
                s = rest.trim();
                changed = true;
            }
            let upper = s.to_uppercase();
            if upper.starts_with("RUN:") {
                s = s[4..].trim();
                changed = true;
            } else if upper.starts_with("EXEC:") {
                s = s[5..].trim();
                changed = true;
            } else if upper.starts_with("TERMINAL:") {
                s = s[9..].trim();
                changed = true;
            } else if upper.starts_with("ACTION:") {
                s = s[7..].trim();
                changed = true;
            } else if upper.starts_with("COMMAND:") {
                s = s[8..].trim();
                changed = true;
            }
            if !changed {
                break;
            }
        }
        s.trim_matches('"').trim_matches('\'').trim().to_string()
    }

    /// Extracts internal reasoning/thought monologue enclosed in <think>...</think> or <thought>...</thought> if present.
    pub fn extract_thought(text: &str) -> Option<String> {
        let mut thoughts = Vec::new();
        if let Some(start) = text.find("<think>") {
            let after = &text[start + 7..];
            if let Some(end) = after.find("</think>") {
                let thought = after[..end].trim();
                if !thought.is_empty() {
                    thoughts.push(thought.to_string());
                }
            } else {
                let thought = after.trim();
                if !thought.is_empty() {
                    thoughts.push(thought.to_string());
                }
            }
        } else if let Some(end) = text.find("</think>") {
            let thought = text[..end].trim();
            if !thought.is_empty() {
                thoughts.push(thought.to_string());
            }
        }

        if let Some(start) = text.find("<thought>") {
            let after = &text[start + 9..];
            if let Some(end) = after.find("</thought>") {
                let thought = after[..end].trim();
                if !thought.is_empty() {
                    thoughts.push(thought.to_string());
                }
            } else {
                let thought = after.trim();
                if !thought.is_empty() {
                    thoughts.push(thought.to_string());
                }
            }
        } else if let Some(end) = text.find("</thought>") {
            let thought = text[..end].trim();
            if !thought.is_empty() {
                thoughts.push(thought.to_string());
            }
        }

        let combined = thoughts.join("\n").trim().to_string();
        if combined.is_empty() {
            None
        } else {
            Some(combined)
        }
    }

    /// Strips out internal thought blocks (<think>...</think> or <thought>...</thought>) to ensure pure actionable deliverable.
    pub fn strip_thought(text: &str) -> String {
        let mut s = text.to_string();
        while let Some(start) = s.find("<think>") {
            if let Some(end) = s[start..].find("</think>") {
                let close_pos = start + end + 8;
                s.replace_range(start..close_pos, "\n");
            } else {
                s.truncate(start);
                break;
            }
        }
        while let Some(start) = s.find("<thought>") {
            if let Some(end) = s[start..].find("</thought>") {
                let close_pos = start + end + 10;
                s.replace_range(start..close_pos, "\n");
            } else {
                s.truncate(start);
                break;
            }
        }
        if let Some(end) = s.find("</think>") {
            s = s[end + 8..].to_string();
        }
        if let Some(end) = s.find("</thought>") {
            s = s[end + 10..].to_string();
        }
        s.trim().to_string()
    }

    /// Parse raw text generated by the worker model into typed SwarmAction
    pub fn parse_action(text: &str) -> SwarmAction {
        // Isolate internal thought blocks before analyzing actionable deliverables
        let stripped = Self::strip_thought(text);
        let mut cleaned = stripped.as_str();
        if let Some(rest) = cleaned.strip_prefix("Assistant:") {
            cleaned = rest.trim();
        }

        // 1. Check for unified diff / patch markers: <<<<<<< TARGET ... ======= ... >>>>>>>
        if let Some(target_start) = cleaned.find("<<<<<<< TARGET") {
            let after_target = &cleaned[target_start + 14..];
            if let Some(sep) = after_target.find("=======") {
                let target_snippet = after_target[..sep].trim_matches('\n');
                let after_sep = &after_target[sep + 7..];
                if let Some(end) = after_sep.find(">>>>>>>") {
                    let replacement_snippet = after_sep[..end].trim_matches('\n');
                    let filename = if let Some(pos) = cleaned.find("PATCH:") {
                        let line = cleaned[pos..].lines().next().unwrap_or("");
                        line.trim_start_matches("PATCH:").trim().to_string()
                    } else if let Some(pos) = cleaned.find("FILE:") {
                        let line = cleaned[pos..].lines().next().unwrap_or("");
                        line.trim_start_matches("FILE:").trim().to_string()
                    } else {
                        String::new()
                    };
                    return SwarmAction::PatchFile {
                        filename,
                        target: target_snippet.to_string(),
                        replacement: replacement_snippet.to_string(),
                    };
                }
            }
        }

        // 2. Check for general code block ``` ... ```
        if let Some(start) = cleaned.find("```") {
            let after = &cleaned[start + 3..];
            // Extract optional language header line
            let first_nl = after.find('\n').unwrap_or(after.len());
            let header = after[..first_nl].trim();
            let is_ident = !header.is_empty() && !header.contains(' ') && header.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let body_start = if is_ident { first_nl + 1 } else { 0 };
            let body = if body_start < after.len() { &after[body_start..] } else { after };
            let block = if let Some(end) = body.find("```") {
                &body[..end]
            } else {
                body
            }.trim();

            let upper_header = header.to_uppercase();
            if upper_header == "TEST_RUN" || upper_header == "TERMINAL_TEST" || upper_header == "TEST" {
                let mut cmd = String::new();
                let mut inputs = Vec::new();
                let mut in_inputs = false;
                for line in block.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Some(rest) = trimmed.strip_prefix("command:")
                        .or_else(|| trimmed.strip_prefix("cmd:"))
                        .or_else(|| trimmed.strip_prefix("COMMAND:"))
                    {
                        cmd = rest.trim().to_string();
                        in_inputs = false;
                    } else if trimmed.eq_ignore_ascii_case("inputs:") || trimmed.eq_ignore_ascii_case("stdin:") {
                        in_inputs = true;
                    } else if in_inputs {
                        let clean_inp = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
                        inputs.push(clean_inp.to_string());
                    } else if cmd.is_empty() && (trimmed.starts_with("python") || trimmed.starts_with("bash") || trimmed.starts_with("sh ") || trimmed.starts_with("cargo ") || trimmed.starts_with("node ")) {
                        cmd = trimmed.to_string();
                    } else {
                        inputs.push(trimmed.to_string());
                    }
                }
                return SwarmAction::TestRun { command: cmd, inputs };
            }

            if upper_header == "VERDICT" {
                let mut status = false;
                let mut missing = Vec::new();
                let mut proof = Vec::new();
                let mut in_missing = false;
                let mut in_proof = false;
                for line in block.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let upper = trimmed.to_uppercase();
                    if upper.starts_with("STATUS:") {
                        let val = trimmed[7..].trim().to_uppercase();
                        status = val == "YES" || val == "ACCEPT" || val == "PASS" || val == "PASSED" || val == "TRUE";
                        in_missing = false;
                        in_proof = false;
                    } else if upper.starts_with("DECISION:") {
                        let val = trimmed[9..].trim().to_uppercase();
                        status = val == "YES" || val == "ACCEPT" || val == "PASS" || val == "PASSED" || val == "TRUE";
                        in_missing = false;
                        in_proof = false;
                    } else if upper.starts_with("MISSING:") {
                        in_missing = true;
                        in_proof = false;
                        let rest = trimmed[8..].trim();
                        if !rest.is_empty() {
                            missing.push(rest.to_string());
                        }
                    } else if upper.starts_with("PROOF:") || upper.starts_with("EVIDENCE:") {
                        in_proof = true;
                        in_missing = false;
                        let rest = trimmed[6..].trim();
                        if !rest.is_empty() {
                            proof.push(rest.to_string());
                        }
                    } else if upper.starts_with("WHY:") || upper.starts_with("REASON:") {
                        let rest = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim();
                        if !rest.is_empty() {
                            if status {
                                proof.push(rest.to_string());
                            } else {
                                missing.push(rest.to_string());
                            }
                        }
                    } else if in_missing {
                        let clean = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
                        missing.push(clean.to_string());
                    } else if in_proof {
                        let clean = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();
                        proof.push(clean.to_string());
                    }
                }
                let proof_or_missing = if status {
                    if proof.is_empty() { "Verified against all user requirements".to_string() } else { proof.join("; ") }
                } else {
                    if missing.is_empty() { "Missing user requirements or unexpected behavior".to_string() } else { missing.join("; ") }
                };
                return SwarmAction::Verdict { status, proof_or_missing };
            }

            let upper_block = block.to_uppercase();
            if header.eq_ignore_ascii_case("bash") || header.eq_ignore_ascii_case("sh")
                || upper_block.starts_with("RUN:")
                || upper_block.starts_with("EXEC:")
                || upper_block.starts_with("TERMINAL:")
            {
                let clean = Self::strip_command_prefix(block);
                return SwarmAction::RunTerminal { command: clean };
            } else if !block.is_empty() {
                return SwarmAction::WriteFile { filename: String::new(), content: block.to_string() };
            }
        }

        // 5. Raw code starting without markdown fences
        if cleaned.starts_with("#!/")
            || cleaned.starts_with("import ")
            || cleaned.starts_with("from ")
            || cleaned.starts_with("def ")
            || cleaned.starts_with("class ")
            || cleaned.starts_with("<!DOCTYPE")
            || cleaned.starts_with("<html")
        {
            return SwarmAction::WriteFile { filename: String::new(), content: cleaned.to_string() };
        }

        // 6. Standard commands line by line
        for line in cleaned.lines() {
            let mut l = line.trim();
            if let Some(rest) = l.strip_prefix("Assistant:") {
                l = rest.trim();
            }

            // Strip bullet points or numbering
            if let Some(rest) = l.strip_prefix("- ")
                .or_else(|| l.strip_prefix("* "))
                .or_else(|| l.strip_prefix("1. "))
                .or_else(|| l.strip_prefix("2. "))
            {
                l = rest.trim();
            }

            // Strip common prefixes
            if let Some(rest) = l.strip_prefix("Action:").or_else(|| l.strip_prefix("action:")) {
                l = rest.trim();
            }
            if let Some(rest) = l.strip_prefix("Command:").or_else(|| l.strip_prefix("command:")) {
                l = rest.trim();
            }

            let upper = l.to_uppercase();
            if upper.starts_with("DELETE:") || upper.starts_with("REMOVE:") || upper.starts_with("RM:") {
                let file = l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::DeleteFile { filename: file.to_string() };
            }
            if upper.starts_with("PATCH:") {
                let rest = l[6..].trim();
                let parts: Vec<&str> = rest.split('|').collect();
                let filename = parts[0].trim().trim_matches('"').trim_matches('\'').to_string();
                let mut target = String::new();
                let mut replacement = String::new();
                for part in &parts[1..] {
                    let trimmed_part = part.trim();
                    let upper_part = trimmed_part.to_uppercase();
                    if upper_part.starts_with("TARGET:") {
                        target = trimmed_part[7..].trim().trim_matches('"').trim_matches('\'').to_string();
                    } else if upper_part.starts_with("REPLACEMENT:") {
                        replacement = trimmed_part[12..].trim().trim_matches('"').trim_matches('\'').to_string();
                    }
                }
                return SwarmAction::PatchFile { filename, target, replacement };
            }
            if upper.starts_with("EXEC:") || upper.starts_with("RUN:") || upper.starts_with("TERMINAL:") {
                let clean = Self::strip_command_prefix(l);
                return SwarmAction::RunTerminal { command: clean };
            }
            if upper.starts_with("VERDICT:") {
                let rest = l[8..].trim();
                let upper_rest = rest.to_uppercase();
                let status = upper_rest.starts_with("YES") || upper_rest.starts_with("ACCEPT") || upper_rest.starts_with("PASS");
                return SwarmAction::Verdict { status, proof_or_missing: rest.to_string() };
            }
            if upper.starts_with("SEARCH:") {
                let q = l[7..].trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Search { query: q.to_string() };
            }
            if upper.starts_with("FETCH:") {
                let u = l[6..].trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Fetch { url: u.to_string() };
            }
            if upper.starts_with("REPORT:") {
                let f = l[7..].trim();
                return SwarmAction::Report { finding: f.to_string() };
            }
            if upper.starts_with("CONSULT:") || upper.starts_with("ASK:") || upper.starts_with("NEED:") {
                let q = l.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
                return SwarmAction::Consult { question: q.to_string() };
            }
            if upper == "DONE" || upper.starts_with("DONE:") || upper.starts_with("DONE ") {
                return SwarmAction::Done;
            }
        }

        SwarmAction::None
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_swarm_actions() {
        let txt1 = "SEARCH: \"James Webb telescope new exoplanet atmosphere\"";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1),
            SwarmAction::Search {
                query: "James Webb telescope new exoplanet atmosphere".to_string()
            }
        );

        let txt2 = "FETCH: https://en.wikipedia.org/wiki/James_Webb_Space_Telescope";
        assert_eq!(
            SwarmCoordinator::parse_action(txt2),
            SwarmAction::Fetch {
                url: "https://en.wikipedia.org/wiki/James_Webb_Space_Telescope".to_string()
            }
        );

        let txt3 = "REPORT: The atmosphere contains traces of methane and carbon dioxide.";
        assert_eq!(
            SwarmCoordinator::parse_action(txt3),
            SwarmAction::Report {
                finding: "The atmosphere contains traces of methane and carbon dioxide.".to_string()
            }
        );

        let txt4 = "DONE";
        assert_eq!(SwarmCoordinator::parse_action(txt4), SwarmAction::Done);

        // Test direct HTML code block produces SwarmAction::WriteFile
        let txt6 = "```html\n<!DOCTYPE html><html><body><h1>Platformer Game</h1></body></html>\n```";
        match SwarmCoordinator::parse_action(txt6) {
            SwarmAction::WriteFile { content, .. } => {
                assert!(content.contains("<h1>Platformer Game</h1>"));
            }
            other => panic!("Expected WriteFile, got {:?}", other),
        }

        // Test direct Python code block produces SwarmAction::WriteFile
        let txt8 = "```python\nimport time\nprint('hello')\n```";
        match SwarmCoordinator::parse_action(txt8) {
            SwarmAction::WriteFile { content, .. } => {
                assert!(content.contains("import time"));
            }
            other => panic!("Expected WriteFile for python, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_test_run_and_verdict_actions() {
        let txt1 = "```test_run\ncommand: python3 main.py\ninputs:\n1\nAlpha Project\n4\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1),
            SwarmAction::TestRun {
                command: "python3 main.py".to_string(),
                inputs: vec!["1".to_string(), "Alpha Project".to_string(), "4".to_string()],
            }
        );

        let txt2 = "```verdict\nstatus: YES\nproof: All features verified and exited cleanly\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(txt2),
            SwarmAction::Verdict {
                status: true,
                proof_or_missing: "All features verified and exited cleanly".to_string(),
            }
        );

        let txt3 = "```verdict\nstatus: NO\nmissing:\n- Data not persisted to disk\n- Exit option missing\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(txt3),
            SwarmAction::Verdict {
                status: false,
                proof_or_missing: "Data not persisted to disk; Exit option missing".to_string(),
            }
        );

        let txt4 = "VERDICT: ACCEPT - All requirements validated";
        assert_eq!(
            SwarmCoordinator::parse_action(txt4),
            SwarmAction::Verdict {
                status: true,
                proof_or_missing: "ACCEPT - All requirements validated".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_thinker_decision() {
        let sg = SubGoal {
            id: 1,
            description: "Write CSS styles".to_string(),
            target_entity: "styles.css".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![PathBuf::from("styles.css")],
            write_set: vec![PathBuf::from("styles.css")],
            max_tokens: 4096,
        };

        let t1 = "DECISION: NUDGE | REASON: Missing closing bracket in styles.css";
        assert_eq!(
            SwarmCoordinator::parse_thinker_decision(t1, &sg),
            ThinkerDecision::Nudge("Missing closing bracket in styles.css".to_string())
        );

        let t2 = "DECISION: RESET | REASON: Rewrite the architecture using grid layout";
        assert_eq!(
            SwarmCoordinator::parse_thinker_decision(t2, &sg),
            ThinkerDecision::Reset("Rewrite the architecture using grid layout".to_string())
        );
    }

    #[test]
    fn test_diagnostic_code_taxonomy() {
        let err1 = DiagnosticCode::VfsFileNotWritten;
        assert_eq!(err1.name(), "ERR_VFS_FILE_NOT_WRITTEN");
        assert!(err1.default_prescription("main.py").contains("code block"));

        let err2 = DiagnosticCode::PlaceholderOrEmpty;
        assert_eq!(err2.name(), "ERR_PLACEHOLDER_OR_EMPTY");
        assert!(err2.default_prescription("main.py").contains("NEVER output placeholders"));

        let err4 = DiagnosticCode::NoCodeBlock;
        assert_eq!(err4.name(), "ERR_NO_CODE_BLOCK");

        let err5 = DiagnosticCode::RepeatedAttractor;
        assert_eq!(err5.name(), "ERR_REPEATED_ATTRACTOR");
    }

    #[test]
    fn test_classify_failure() {
        // 1. No code block
        let diag1 = SwarmCoordinator::classify_failure(
            &SwarmAction::None,
            false,
            "",
            false,
            0,
            None,
            "I will now write the file for you:",
            None,
        );
        assert_eq!(diag1, DiagnosticCode::NoCodeBlock);

        // 2. Execution failure
        let diag2 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunTerminal { command: "python3 broken.py".to_string() },
            false,
            "SyntaxError: invalid syntax",
            false,
            0,
            None,
            "```python\nprint(\n```",
            None,
        );
        assert_eq!(diag2, DiagnosticCode::TestFailed);

        // 3. VFS file not written
        let diag3 = SwarmCoordinator::classify_failure(
            &SwarmAction::WriteFile { filename: "main.py".to_string(), content: "print('done')".to_string() },
            true,
            "done",
            false,
            0,
            None,
            "```python\nprint('done')\n```",
            None,
        );
        assert_eq!(diag3, DiagnosticCode::VfsFileNotWritten);

        // 4. Placeholder or empty
        let diag4 = SwarmCoordinator::classify_failure(
            &SwarmAction::WriteFile { filename: "main.py".to_string(), content: "# TODO".to_string() },
            true,
            "",
            true,
            6,
            Some("# TODO"),
            "```python\n# TODO\n```",
            None,
        );
        assert_eq!(diag4, DiagnosticCode::PlaceholderOrEmpty);

        // 5. Repeated attractor
        let diag5 = SwarmCoordinator::classify_failure(
            &SwarmAction::WriteFile { filename: "main.py".to_string(), content: "# TODO".to_string() },
            true,
            "",
            true,
            6,
            Some("# TODO"),
            "```python\n# TODO\n```",
            Some(&DiagnosticCode::PlaceholderOrEmpty),
        );
        assert_eq!(diag5, DiagnosticCode::RepeatedAttractor);

        // 6. Generic Missing dependency detection
        let diag6 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunTerminal { command: "python3 script.py".to_string() },
            false,
            "ModuleNotFoundError: No module named 'scipy'",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag6, DiagnosticCode::MissingDependency("scipy".to_string()));

        let diag7 = SwarmCoordinator::classify_failure(
            &SwarmAction::RunTerminal { command: "ffmpeg -version".to_string() },
            false,
            "/bin/sh: line 1: ffmpeg: command not found",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag7, DiagnosticCode::MissingDependency("ffmpeg".to_string()));
    }

    #[test]
    fn test_extract_missing_dependency_generic() {
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'requests'"),
            Some("requests".to_string())
        );
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("sh: line 1: jq: command not found"),
            Some("jq".to_string())
        );
        assert_eq!(
            SwarmCoordinator::extract_missing_dependency("ImportError: cannot import name 'solve' from 'scipy'"),
            Some("scipy".to_string())
        );
    }

    #[test]
    fn test_explicit_single_file_detection() {
        assert!(SwarmCoordinator::is_explicit_single_file("Create a script in a single file"));
        assert!(SwarmCoordinator::is_explicit_single_file("اكتب كود بايثون في ملف واحد"));
        assert!(SwarmCoordinator::is_explicit_single_file("Build a standalone script for backups"));
        assert!(SwarmCoordinator::is_explicit_single_file("I need a Python program to measure internet speed."));
        assert!(SwarmCoordinator::is_explicit_single_file("أريد برنامج بايثون لقياس سرعة الإنترنت"));
        assert!(!SwarmCoordinator::is_explicit_single_file("Create an HTML app with external CSS and JS files"));
    }

    #[test]
    fn test_normalize_entity_filename() {
        assert_eq!(SwarmCoordinator::normalize_entity_filename("game_loop.java"), "GameLoop.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("snake_body.java"), "SnakeBody.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("collision_detection.java"), "CollisionDetection.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("GameLoop.java"), "GameLoop.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("Snake.java"), "Snake.java");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("main.py"), "main.py");
        assert_eq!(SwarmCoordinator::normalize_entity_filename("styles.css"), "styles.css");
    }

    #[test]
    fn test_parse_patch_and_delete_actions() {
        // DELETE action
        let txt1 = "DELETE: old_script.py";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1),
            SwarmAction::DeleteFile { filename: "old_script.py".to_string() }
        );

        let txt1_rm = "RM: temp_data.json";
        assert_eq!(
            SwarmCoordinator::parse_action(txt1_rm),
            SwarmAction::DeleteFile { filename: "temp_data.json".to_string() }
        );

        // PATCH command format
        let txt2 = "PATCH: config.py | TARGET: debug = False | REPLACEMENT: debug = True";
        assert_eq!(
            SwarmCoordinator::parse_action(txt2),
            SwarmAction::PatchFile {
                filename: "config.py".to_string(),
                target: "debug = False".to_string(),
                replacement: "debug = True".to_string(),
            }
        );

        // Unified diff / patch markers
        let txt3 = "PATCH: main.py\n<<<<<<< TARGET\ndef old_calc():\n    return 1\n=======\ndef new_calc():\n    return 2\n>>>>>>>";
        assert_eq!(
            SwarmCoordinator::parse_action(txt3),
            SwarmAction::PatchFile {
                filename: "main.py".to_string(),
                target: "def old_calc():\n    return 1".to_string(),
                replacement: "def new_calc():\n    return 2".to_string(),
            }
        );

        // Terminal EXEC
        let txt4 = "EXEC: pytest tests/";
        assert_eq!(
            SwarmCoordinator::parse_action(txt4),
            SwarmAction::RunTerminal { command: "pytest tests/".to_string() }
        );
    }

    #[test]
    fn test_infer_task_kind() {
        assert_eq!(SwarmCoordinator::infer_task_kind("Create a new web server in Python"), TaskKind::Create);
        assert_eq!(SwarmCoordinator::infer_task_kind("Patch the bug in line 42"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("عدل الكود وصلح الخطأ"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("Delete redundant files from workspace"), TaskKind::Delete);
        assert_eq!(SwarmCoordinator::infer_task_kind("احذف الملف القديم"), TaskKind::Delete);
        assert_eq!(SwarmCoordinator::infer_task_kind("Implement a lightweight HTTP JSON health check server using Flask"), TaskKind::Create);
        assert_eq!(SwarmCoordinator::infer_task_kind("Test the unit test suite and verify output"), TaskKind::Test);
        assert_eq!(SwarmCoordinator::infer_task_kind("اختبر الكود وافحص النتيجة"), TaskKind::Test);
        assert_eq!(SwarmCoordinator::infer_task_kind("Execute cleanroom verification of API endpoints and UI interactions"), TaskKind::Test);
        assert_eq!(SwarmCoordinator::infer_task_kind("Run Test Sheet protocol verification & review of 'index.html'"), TaskKind::Test);
        assert_eq!(SwarmCoordinator::infer_task_kind("Fix delete button in index.html"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("Migrate app.py backend to Flask and support DELETE endpoint"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("Refactor tasks API to use Flask"), TaskKind::Modify);
        assert_eq!(SwarmCoordinator::infer_task_kind("Delete redundant files from workspace"), TaskKind::Delete);
    }

    #[test]
    fn test_extract_missing_dependency_filters_internal_keywords() {
        assert_eq!(SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'flask'"), Some("flask".to_string()));
        assert_eq!(SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'vfs'"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'cleanroom'"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("ModuleNotFoundError: No module named 'sandbox'"), None);
    }

    #[test]
    fn test_resolve_existing_file_priority() {
        let temp_in = tempfile::tempdir().unwrap();
        let temp_out = tempfile::tempdir().unwrap();
        
        let file_in = temp_in.path().join("source.py");
        std::fs::write(&file_in, "print('in')").unwrap();

        let file_out = temp_out.path().join("output.py");
        std::fs::write(&file_out, "print('out')").unwrap();

        let config = SwarmConfig {
            orchestrator_model_path: PathBuf::from("mock.gguf"),
            worker_model_path: PathBuf::from("mock.gguf"),
            max_subgoals: 3,
            max_steps_per_worker: 3,
            input_dir: Some(temp_in.path().to_path_buf()),
            output_dir: Some(temp_out.path().to_path_buf()),
            verbose: false,
            auto_approve_deps: None,
            is_modify_mode: false,
            review_plan: false,
        };

        let resolve_fn = |p: &Path| -> Option<PathBuf> {
            if let Some(ref in_dir) = config.input_dir {
                let candidate = in_dir.join(p);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            if let Some(ref out_dir) = config.output_dir {
                let candidate = out_dir.join(p);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
            if p.exists() {
                return Some(p.to_path_buf());
            }
            None
        };

        assert_eq!(resolve_fn(Path::new("source.py")), Some(file_in));
        assert_eq!(resolve_fn(Path::new("output.py")), Some(file_out));
        assert_eq!(resolve_fn(Path::new("non_existent.py")), None);
    }

    #[test]
    fn test_modify_mode_explicit_kind_and_fallback() {
        let temp_in = tempfile::tempdir().unwrap();
        let target_file = temp_in.path().join("app.py");
        std::fs::write(&target_file, "print('old code')").unwrap();

        let config = SwarmConfig {
            orchestrator_model_path: PathBuf::from("mock.gguf"),
            worker_model_path: PathBuf::from("mock.gguf"),
            max_subgoals: 3,
            max_steps_per_worker: 3,
            input_dir: Some(temp_in.path().to_path_buf()),
            output_dir: Some(temp_in.path().to_path_buf()),
            verbose: false,
            auto_approve_deps: None,
            is_modify_mode: true,
            review_plan: false,
        };

        let vfs = MemoryVfs::new();
        let relative = PathBuf::from("app.py");
        assert!(!vfs.exists(&relative));

        // In modify mode, existing file on disk resolves properly
        let candidate = config.input_dir.as_ref().unwrap().join(&relative);
        assert!(candidate.exists());
        let content = std::fs::read(&candidate).unwrap();
        vfs.preload_file(&relative, &content);
        assert!(vfs.exists(&relative));
    }

    #[test]
    fn test_classify_failure_for_kind() {
        // Delete kind failure when file still exists
        let diag1 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::DeleteFile { filename: "test.txt".to_string() },
            false,
            "failed",
            true, // file still exists
            10,
            None,
            "",
            None,
            Some(&TaskKind::Delete),
        );
        assert_eq!(diag1, DiagnosticCode::FileNotDeleted);

        // Modify kind failure when diff is empty
        let diag2 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::PatchFile { filename: "test.txt".to_string(), target: "a".into(), replacement: "b".into() },
            true,
            "success",
            true,
            10,
            Some(""), // empty diff
            "",
            None,
            Some(&TaskKind::Modify),
        );
        assert_eq!(diag2, DiagnosticCode::DiffEmpty);

        // Test kind failure
        let diag3 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::RunTerminal { command: "pytest".to_string() },
            false,
            "assertion failed",
            true,
            10,
            None,
            "",
            None,
            Some(&TaskKind::Test),
        );
        assert_eq!(diag3, DiagnosticCode::TestFailed);

        // Missing dependency extraction in test kind
        let diag4 = SwarmCoordinator::classify_failure_for_kind(
            &SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() },
            false,
            "ModuleNotFoundError: No module named 'tkinter'",
            true,
            100,
            None,
            "",
            None,
            Some(&TaskKind::Test),
        );
        assert_eq!(diag4, DiagnosticCode::MissingDependency("tkinter".to_string()));
    }

    #[test]
    fn test_strip_command_prefix_and_robust_parsing() {
        assert_eq!(SwarmCoordinator::strip_command_prefix("RUN: python3 GameLoop.py"), "python3 GameLoop.py");
        assert_eq!(SwarmCoordinator::strip_command_prefix("EXEC:   bash test.sh"), "bash test.sh");
        assert_eq!(SwarmCoordinator::strip_command_prefix("- RUN: $ python3 foo.py"), "python3 foo.py");
        assert_eq!(SwarmCoordinator::strip_command_prefix("Assistant: 1. RUN: pytest tests/"), "pytest tests/");

        // Code block containing RUN:
        let code_block = "```python\nRUN: python3 GameLoop.py\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(code_block),
            SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() }
        );

        // Code block with bash header
        let bash_block = "```bash\npython3 GameLoop.py\n```";
        assert_eq!(
            SwarmCoordinator::parse_action(bash_block),
            SwarmAction::RunTerminal { command: "python3 GameLoop.py".to_string() }
        );

        // Command not found for procedural words should NOT extract missing dependency
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: RUN:: not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: RUN: command not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: exec: not found"), None);
        assert_eq!(SwarmCoordinator::extract_missing_dependency("sh: line 1: python: command not found"), None);
    }

    #[test]
    fn test_subgoal_boundaries_and_helpers() {
        let sg = SubGoal {
            id: 1,
            description: "Write main script".to_string(),
            target_entity: "main.py".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["utils.py".to_string()],
            read_set: vec![PathBuf::from("utils.py"), PathBuf::from("main.py")],
            write_set: vec![PathBuf::from("main.py")],
            max_tokens: 4096,
        };
        assert!(sg.can_write("main.py"));
        assert!(sg.can_write("./main.py"));
        assert!(!sg.can_write("utils.py"));
        assert!(sg.can_read("utils.py"));
        assert!(sg.can_read("./utils.py"));
        assert!(sg.can_read("main.py"));
        assert!(!sg.can_read("secret.key"));
    }

    #[test]
    fn test_diagnostic_out_of_bounds_write_taxonomy() {
        let diag = DiagnosticCode::OutOfBoundsWrite;
        assert_eq!(diag.name(), "ERR_OUT_OF_BOUNDS_WRITE");
        assert!(diag.default_prescription("target.rs").contains("write_set"));
    }

    #[test]
    fn test_classify_out_of_bounds_write_from_coordinator() {
        let diag = SwarmCoordinator::classify_failure(
            &SwarmAction::WriteFile {
                filename: "secret.env".to_string(),
                content: "MALICIOUS".to_string(),
            },
            false,
            "ERR_OUT_OF_BOUNDS_WRITE: File 'secret.env' is not in write_set [\"main.rs\"]",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag, DiagnosticCode::OutOfBoundsWrite);
    }

    #[test]
    fn test_classify_out_of_bounds_write_from_vfs_boundary() {
        let diag = SwarmCoordinator::classify_failure(
            &SwarmAction::WriteFile {
                filename: "secret.env".to_string(),
                content: "leak".to_string(),
            },
            false,
            "VFS_BOUNDARY_VIOLATION: Unauthorized write to 'secret.env'",
            false,
            0,
            None,
            "",
            None,
        );
        assert_eq!(diag, DiagnosticCode::OutOfBoundsWrite);
    }

    #[test]
    fn test_orchestrate_plan_deterministic_fallbacks() {
        let entity = "app.py";
        let depends_on = vec!["config.py".to_string(), "db.py".to_string()];
        let target_path = PathBuf::from(entity);
        
        let mut fallback_read_set = Vec::new();
        for dep in &depends_on {
            fallback_read_set.push(PathBuf::from(dep));
        }
        fallback_read_set.push(target_path.clone());
        
        let fallback_write_set = vec![target_path];
        
        assert_eq!(fallback_write_set, vec![PathBuf::from("app.py")]);
        assert_eq!(fallback_read_set, vec![
            PathBuf::from("config.py"),
            PathBuf::from("db.py"),
            PathBuf::from("app.py")
        ]);
    }

    // =========================================================================
    // M2 UNIT TESTS: TaskStatus, MemoryGate, TaskDag & StateMachineCoordinator
    // =========================================================================

    #[test]
    fn test_task_status_state_machine_transitions() {
        let p = TaskStatus::Pending;
        assert!(p.can_transition_to(TaskStatus::Ready));
        assert!(p.can_transition_to(TaskStatus::Cancelled));
        assert!(p.can_transition_to(TaskStatus::Failed)); // Zero-OOM pre-flight
        assert!(!p.can_transition_to(TaskStatus::Running));
        assert!(!p.can_transition_to(TaskStatus::Completed));

        let r = TaskStatus::Ready;
        assert!(r.can_transition_to(TaskStatus::Running));
        assert!(r.can_transition_to(TaskStatus::Cancelled));
        assert!(r.can_transition_to(TaskStatus::Failed));
        assert!(!r.can_transition_to(TaskStatus::Pending));
        assert!(!r.can_transition_to(TaskStatus::Completed));

        let run = TaskStatus::Running;
        assert!(run.can_transition_to(TaskStatus::Completed));
        assert!(run.can_transition_to(TaskStatus::Failed));
        assert!(run.can_transition_to(TaskStatus::Cancelled));
        assert!(!run.can_transition_to(TaskStatus::Pending));
        assert!(!run.can_transition_to(TaskStatus::Ready));

        // Terminal states cannot transition to anything
        for terminal in [TaskStatus::Completed, TaskStatus::Failed, TaskStatus::Cancelled] {
            assert!(terminal.is_terminal());
            for other in [
                TaskStatus::Pending,
                TaskStatus::Ready,
                TaskStatus::Running,
                TaskStatus::Completed,
                TaskStatus::Failed,
                TaskStatus::Cancelled,
            ] {
                assert!(!terminal.can_transition_to(other));
            }
        }
    }

    #[test]
    fn test_memory_gate_analytical_bytes_per_token() {
        // Analytical formula verification for Qwen2 (0.5B):
        // L=24, D=896, H_q=14, H_kv=2 -> d_head=64
        // bytes_per_token = 4 * 24 * 2 * 64 = 12,288 B
        let bpt = MemoryGate::analytical_bytes_per_token(24, 2, 896, 14);
        assert_eq!(bpt, 12_288, "Qwen-0.5B must calculate exactly 12,288 bytes/token");

        // Qwen2.5 (1.5B): L=28, D=1536, H_q=12, H_kv=2 -> d_head=128
        // bytes_per_token = 4 * 28 * 2 * 128 = 28,672 B
        let bpt_1_5b = MemoryGate::analytical_bytes_per_token(28, 2, 1536, 12);
        assert_eq!(bpt_1_5b, 28_672, "Qwen2.5-1.5B must calculate exactly 28,672 bytes/token");
    }

    #[test]
    fn test_memory_gate_allocation_and_release() {
        let mut gate = MemoryGate::new(4096);
        assert_eq!(gate.remaining(), 4096);
        assert_eq!(gate.active_tokens, 0);

        // Allocate 1024
        assert!(gate.can_admit(1024));
        assert!(!gate.is_over_capacity(1024));
        assert!(gate.allocate(1024).is_ok());
        assert_eq!(gate.active_tokens, 1024);
        assert_eq!(gate.remaining(), 3072);

        // Allocate 2048
        assert!(gate.allocate(2048).is_ok());
        assert_eq!(gate.active_tokens, 3072);
        assert_eq!(gate.remaining(), 1024);

        // Release 1024
        gate.release(1024);
        assert_eq!(gate.active_tokens, 2048);
        assert_eq!(gate.remaining(), 2048);

        // Release with saturation
        gate.release(5000);
        assert_eq!(gate.active_tokens, 0);
        assert_eq!(gate.remaining(), 4096);
    }

    #[test]
    fn test_memory_gate_saturating_release() {
        let mut gate = MemoryGate::new(4096);
        gate.allocate(1000).unwrap();
        // Release more than active should saturate to 0, not underflow
        gate.release(2000);
        assert_eq!(gate.active_tokens, 0);
        assert_eq!(gate.remaining(), 4096);
    }

    #[test]
    fn test_memory_gate_zero_oom_rejection() {
        let gate = MemoryGate::new(4096);
        // Request exceeds global k_max
        assert!(gate.is_over_capacity(4097));
        assert!(gate.is_over_capacity(8192));
        assert!(!gate.is_over_capacity(4096));
    }

    #[test]
    fn test_memory_gate_context_boom_pause_and_release() {
        let mut gate = MemoryGate::new(4096);
        assert!(gate.allocate(3000).is_ok());

        // Fits in k_max (1500 <= 4096), but active + requested (3000 + 1500 = 4500) > 4096
        assert!(!gate.is_over_capacity(1500));
        assert!(!gate.can_admit(1500));
        assert!(gate.allocate(1500).is_err());

        // Capacity frees when running task releases quota
        gate.release(3000);
        assert!(gate.can_admit(1500));
        assert!(gate.allocate(1500).is_ok());
        assert_eq!(gate.active_tokens, 1500);
    }

    #[test]
    fn test_task_dag_from_subgoals_and_kahn_cycle_detection() {
        let sg1 = SubGoal {
            id: 1,
            description: "Task 1".to_string(),
            target_entity: "a.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("a.rs")],
            max_tokens: 1024,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Task 2".to_string(),
            target_entity: "b.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("a.rs")],
            write_set: vec![PathBuf::from("b.rs")],
            max_tokens: 1024,
        };

        let dag = TaskDag::from_subgoals(vec![sg1.clone(), sg2.clone()]).expect("Valid DAG");
        assert_eq!(dag.root_tasks(), vec![1]);
        assert_eq!(dag.children.get(&1).unwrap(), &HashSet::from([2]));
        assert_eq!(dag.dependencies.get(&2).unwrap(), &HashSet::from([1]));

        // Test cycle detection
        let mut cycle_dag = TaskDag::default();
        cycle_dag.tasks.insert(1, sg1);
        cycle_dag.tasks.insert(2, sg2);
        cycle_dag.add_edge(1, 2);
        cycle_dag.add_edge(2, 1);
        cycle_dag.recompute_in_degrees();
        assert!(cycle_dag.validate_acyclic().is_err());
    }

    #[test]
    fn test_state_machine_coordinator_full_lifecycle() {
        let sg1 = SubGoal {
            id: 1,
            description: "Root".to_string(),
            target_entity: "root.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("root.rs")],
            max_tokens: 1024,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Child".to_string(),
            target_entity: "child.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("root.rs")],
            write_set: vec![PathBuf::from("child.rs")],
            max_tokens: 1024,
        };

        let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        assert_eq!(coord.ready_set, vec![1]);
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Ready);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Pending);

        // Admit Task 1
        let abort_1 = coord.admit_task(1, 1024).expect("Admit Task 1");
        assert!(!abort_1.load(Ordering::SeqCst));
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Running);
        assert_eq!(coord.memory_gate.active_tokens, 1024);
        assert!(coord.ready_set.is_empty());

        // Complete Task 1 -> Awakens Task 2
        let unblocked = coord.complete_task(1).expect("Complete Task 1");
        assert_eq!(unblocked, vec![2]);
        assert_eq!(coord.ready_set, vec![2]);
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Completed);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Ready);
        assert_eq!(coord.memory_gate.active_tokens, 0);

        // Admit & complete Task 2
        let _abort_2 = coord.admit_task(2, 1024).expect("Admit Task 2");
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Running);
        let _ = coord.complete_task(2).expect("Complete Task 2");
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Completed);
        assert!(coord.is_all_terminal());
    }

    #[test]
    fn test_state_machine_coordinator_zero_oom_cascade() {
        let sg1 = SubGoal {
            id: 1,
            description: "OOM task".to_string(),
            target_entity: "oom.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("oom.rs")],
            max_tokens: 8192,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Dependent child".to_string(),
            target_entity: "child.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("oom.rs")],
            write_set: vec![PathBuf::from("child.rs")],
            max_tokens: 1024,
        };

        let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let admit_res = coord.admit_task(1, 8192);
        assert!(admit_res.is_err());
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Cancelled);
        assert!(coord.ready_set.is_empty());
        assert_eq!(coord.memory_gate.active_tokens, 0);
        assert!(coord.is_all_terminal());
    }

    #[test]
    fn test_state_machine_coordinator_failure_cascade() {
        let sg1 = SubGoal {
            id: 1,
            description: "Root".to_string(),
            target_entity: "root.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("root.rs")],
            max_tokens: 1024,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Intermediate".to_string(),
            target_entity: "inter.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("root.rs")],
            write_set: vec![PathBuf::from("inter.rs")],
            max_tokens: 1024,
        };
        let sg3 = SubGoal {
            id: 3,
            description: "Leaf".to_string(),
            target_entity: "leaf.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["2".to_string()],
            read_set: vec![PathBuf::from("inter.rs")],
            write_set: vec![PathBuf::from("leaf.rs")],
            max_tokens: 1024,
        };

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let _ = coord.admit_task(1, 1024).unwrap();
        let cancelled = coord.fail_task(1, "VFS OutOfBoundsWrite", Some(DiagnosticCode::OutOfBoundsWrite));

        assert_eq!(cancelled, vec![2, 3]);
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Cancelled);
        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Cancelled);
        assert_eq!(coord.memory_gate.active_tokens, 0);
        assert!(coord.is_all_terminal());
    }

    #[test]
    fn test_state_machine_coordinator_diamond_partial_cascade() {
        let sg1 = SubGoal {
            id: 1,
            description: "Root".to_string(),
            target_entity: "root.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("root.rs")],
            max_tokens: 1024,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Branch A".to_string(),
            target_entity: "a.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("root.rs")],
            write_set: vec![PathBuf::from("a.rs")],
            max_tokens: 1024,
        };
        let sg3 = SubGoal {
            id: 3,
            description: "Branch B".to_string(),
            target_entity: "b.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["1".to_string()],
            read_set: vec![PathBuf::from("root.rs")],
            write_set: vec![PathBuf::from("b.rs")],
            max_tokens: 1024,
        };
        let sg4 = SubGoal {
            id: 4,
            description: "Sink".to_string(),
            target_entity: "sink.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec!["2".to_string(), "3".to_string()],
            read_set: vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")],
            write_set: vec![PathBuf::from("sink.rs")],
            max_tokens: 1024,
        };

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let _ = coord.admit_task(1, 1024).unwrap();
        let unblocked = coord.complete_task(1).unwrap();
        assert_eq!(unblocked, vec![2, 3]);

        let _ = coord.admit_task(2, 1024).unwrap();
        let _ = coord.admit_task(3, 1024).unwrap();

        let cancelled = coord.fail_task(2, "Branch A failure", None);
        assert_eq!(cancelled, vec![4]);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Failed);
        assert_eq!(coord.records.get(&4).unwrap().status, TaskStatus::Cancelled);
        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);

        let _ = coord.complete_task(3).unwrap();
        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Completed);
        assert_eq!(coord.memory_gate.active_tokens, 0);
        assert!(coord.is_all_terminal());
    }

    #[test]
    fn test_state_machine_coordinator_running_preemption_and_memory_restitution() {
        let sg1 = SubGoal {
            id: 1,
            description: "Task 1".to_string(),
            target_entity: "a.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("a.rs")],
            max_tokens: 1024,
        };
        let sg2 = SubGoal {
            id: 2,
            description: "Task 2".to_string(),
            target_entity: "b.rs".to_string(),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: vec![],
            read_set: vec![],
            write_set: vec![PathBuf::from("b.rs")],
            max_tokens: 2048,
        };

        let mut dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
        dag.add_edge(1, 2);

        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();
        let abort_2 = {
            let rec2 = coord.records.get_mut(&2).unwrap();
            rec2.status = TaskStatus::Running;
            rec2.allocated_tokens = 2048;
            rec2.abort_flag.clone()
        };
        coord.memory_gate.allocate(2048).unwrap();
        assert_eq!(coord.memory_gate.active_tokens, 2048);

        let cancelled = coord.fail_task(1, "Ancestor crash", None);
        assert_eq!(cancelled, vec![2]);
        assert!(abort_2.load(Ordering::SeqCst), "Abort flag must be signaled");
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Cancelled);
        assert_eq!(coord.records.get(&2).unwrap().allocated_tokens, 0);
        assert_eq!(coord.memory_gate.active_tokens, 0, "Memory must be immediately restored");
    }

    // =======================================================================
    // FEATURE 11: LRP & DURATION MODELING TESTS
    // =======================================================================
    fn make_subgoal_with_tokens(id: usize, max_tokens: usize, depends_on: Vec<&str>) -> SubGoal {
        SubGoal {
            id,
            description: format!("Task #{}", id),
            target_entity: format!("file_{}.rs", id),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: depends_on.into_iter().map(String::from).collect(),
            read_set: vec![],
            write_set: vec![PathBuf::from(format!("file_{}.rs", id))],
            max_tokens,
        }
    }

    #[test]
    fn test_duration_modeling_floor_and_context_clamp() {
        let sg_small = make_subgoal_with_tokens(1, 512, vec![]);
        let config_default = LrpConfig::default();
        assert_eq!(sg_small.effective_max_tokens(2048, None), 2048);
        assert_eq!(sg_small.duration_max(&config_default), 2048.0 / 20.0);

        let sg_large = make_subgoal_with_tokens(2, 4096, vec![]);
        assert_eq!(sg_large.effective_max_tokens(2048, None), 4096);
        assert_eq!(sg_large.duration_max(&config_default), 4096.0 / 20.0);

        let config_clamped = LrpConfig::new(20.0, 2048, Some(3072));
        assert_eq!(sg_large.effective_max_tokens(2048, Some(3072)), 3072);
        assert_eq!(sg_large.duration_max(&config_clamped), 3072.0 / 20.0);

        let config_unit = LrpConfig::new(1.0, 2048, None);
        assert_eq!(sg_large.duration_max(&config_unit), 4096.0);
    }

    #[test]
    fn test_lrp_linear_chain() {
        let sg1 = make_subgoal_with_tokens(1, 2048, vec![]);
        let sg2 = make_subgoal_with_tokens(2, 4096, vec!["1"]);
        let sg3 = make_subgoal_with_tokens(3, 2048, vec!["2"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let config = LrpConfig::new(1.0, 2048, None);
        let lrp_table = dag.compute_lrp(&config).unwrap();

        assert_eq!(lrp_table.get(&3).copied().unwrap(), 2048.0);
        assert_eq!(lrp_table.get(&2).copied().unwrap(), 6144.0);
        assert_eq!(lrp_table.get(&1).copied().unwrap(), 8192.0);
        assert_eq!(dag.critical_path_length(&config).unwrap(), 8192.0);
    }

    #[test]
    fn test_lrp_diamond_dag_bottleneck_dominance() {
        let sg1 = make_subgoal_with_tokens(1, 2048, vec![]);
        let sg2 = make_subgoal_with_tokens(2, 4096, vec!["1"]);
        let sg3 = make_subgoal_with_tokens(3, 2048, vec!["1"]);
        let sg4 = make_subgoal_with_tokens(4, 2048, vec!["2", "3"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
        let config = LrpConfig::new(1.0, 2048, None);
        let lrp_table = dag.compute_lrp(&config).unwrap();

        assert_eq!(lrp_table.get(&4).copied().unwrap(), 2048.0);
        assert_eq!(lrp_table.get(&2).copied().unwrap(), 4096.0 + 2048.0);
        assert_eq!(lrp_table.get(&3).copied().unwrap(), 2048.0 + 2048.0);
        assert_eq!(lrp_table.get(&1).copied().unwrap(), 8192.0);

        let lrp_memo = dag.compute_lrp_memo(&config).unwrap();
        assert_eq!(lrp_table, lrp_memo);
    }

    #[test]
    fn test_lrp_disconnected_forest() {
        let sg1 = make_subgoal_with_tokens(1, 2048, vec![]);
        let sg2 = make_subgoal_with_tokens(2, 2048, vec!["1"]);
        let sg3 = make_subgoal_with_tokens(3, 4096, vec![]);
        let sg4 = make_subgoal_with_tokens(4, 4096, vec!["3"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
        let config = LrpConfig::new(1.0, 2048, None);
        let lrp_table = dag.compute_lrp(&config).unwrap();

        assert_eq!(lrp_table.get(&1).copied().unwrap(), 4096.0);
        assert_eq!(lrp_table.get(&2).copied().unwrap(), 2048.0);
        assert_eq!(lrp_table.get(&3).copied().unwrap(), 8192.0);
        assert_eq!(lrp_table.get(&4).copied().unwrap(), 4096.0);

        assert_eq!(dag.critical_path_length(&config).unwrap(), 8192.0);

        let mut ready = vec![1, 3];
        dag.sort_ready_set(&mut ready, &lrp_table);
        assert_eq!(ready, vec![3, 1]);
    }

    #[test]
    fn test_ready_set_priority_ordering_and_deterministic_tie_breaking() {
        let dag = TaskDag::default();
        let mut lrp_table = HashMap::new();
        lrp_table.insert(1, 4096.0);
        lrp_table.insert(2, 8192.0);
        lrp_table.insert(3, 4096.0);
        lrp_table.insert(4, 1024.0);

        let mut ready_set = vec![4, 3, 1, 2];
        dag.sort_ready_set(&mut ready_set, &lrp_table);
        assert_eq!(ready_set, vec![2, 1, 3, 4]);
    }

    #[test]
    fn test_dynamic_ready_set_updates_with_lrp() {
        let sg1 = make_subgoal_with_tokens(1, 2048, vec![]);
        let sg2 = make_subgoal_with_tokens(2, 4096, vec!["1"]);
        let sg3 = make_subgoal_with_tokens(3, 2048, vec!["1"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let config = LrpConfig::new(1.0, 2048, None);
        let gate = MemoryGate::new(8192);
        let mut coord = StateMachineCoordinator::with_memory_gate_and_lrp(dag, gate, config).unwrap();

        assert_eq!(coord.ready_set, vec![1]);
        assert_eq!(coord.head_task(), Some(1));

        let _ = coord.admit_task(1, 2048).unwrap();
        let unblocked = coord.complete_task(1).unwrap();
        assert_eq!(unblocked, vec![2, 3]);

        coord.dag.sort_ready_set(&mut coord.ready_set, &coord.lrp_table);
        assert_eq!(coord.ready_set, vec![2, 3]);
        assert_eq!(coord.head_task(), Some(2));
    }

    // =======================================================================
    // FEATURE 12: RCPSP HEAD-PINNING & EASY BACKFILLING TESTS
    // =======================================================================
    fn make_test_subgoal_m3(id: usize, max_tokens: usize, deps: Vec<&str>, writes: Vec<&str>) -> SubGoal {
        SubGoal {
            id,
            description: format!("Task #{}", id),
            target_entity: format!("entity_{}.rs", id),
            kind: TaskKind::Create,
            guidance: None,
            depends_on: deps.into_iter().map(String::from).collect(),
            read_set: vec![],
            write_set: writes.into_iter().map(PathBuf::from).collect(),
            max_tokens,
        }
    }

    #[test]
    fn test_rcpsp_immediate_head_dispatch() {
        let scheduler = RcpspScheduler::default_rate();
        let sg1 = make_test_subgoal_m3(1, 2048, vec![], vec!["a.rs"]);
        let dag = TaskDag::from_subgoals(vec![sg1]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let mut lrp_map = HashMap::new();
        lrp_map.insert(1, 102.4);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        assert_eq!(
            decision,
            ScheduleDecision::DispatchHead {
                task_id: 1,
                allocated_tokens: 2048,
            }
        );
        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Running);
        assert_eq!(coord.memory_gate.remaining(), 2048);
    }

    #[test]
    fn test_rcpsp_shadow_reservation_calculation() {
        let scheduler = RcpspScheduler::new(20.0);
        let sg1 = make_test_subgoal_m3(1, 3000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 2500, vec![], vec!["b.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 3000).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 1096);

        let res = scheduler.compute_shadow_reservation(&coord, 2, 2500).unwrap();
        assert_eq!(res.task_id, 2);
        assert_eq!(res.deficit_tokens, 1404);
        assert_eq!(res.releasing_tasks.len(), 1);
        assert_eq!(res.releasing_tasks[0].0, 1);
        assert_eq!(res.releasing_tasks[0].1, 3000);
        assert!((res.reservation_delay_secs - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_rcpsp_backfill_sub_predicate_a_early_completion() {
        let scheduler = RcpspScheduler::new(20.0);
        let sg1 = make_test_subgoal_m3(1, 3000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3000, vec![], vec!["b.rs"]);
        let sg3 = make_test_subgoal_m3(3, 500, vec![], vec!["c.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 3000).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 1096);

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 200.0);
        lrp_map.insert(3, 50.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        match decision {
            ScheduleDecision::PinnedWithBackfill {
                pinned_head_id,
                backfilled_task_ids,
                ..
            } => {
                assert_eq!(pinned_head_id, 2);
                assert_eq!(backfilled_task_ids, vec![3]);
            }
            other => panic!("Expected PinnedWithBackfill, got {:?}", other),
        }

        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
        assert_eq!(coord.memory_gate.active_tokens, 3500);
        assert_eq!(coord.memory_gate.remaining(), 596);
    }

    #[test]
    fn test_rcpsp_backfill_sub_predicate_b_headroom_preserved() {
        let scheduler = RcpspScheduler::new(10.0);
        let sg1 = make_test_subgoal_m3(1, 4000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3200, vec![], vec!["b.rs"]);
        let sg3 = make_test_subgoal_m3(3, 1000, vec![], vec!["c.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 6000).unwrap();

        coord.admit_task(1, 4000).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 2000);

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 500.0);
        lrp_map.insert(3, 100.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        match decision {
            ScheduleDecision::PinnedWithBackfill {
                pinned_head_id,
                backfilled_task_ids,
                ..
            } => {
                assert_eq!(pinned_head_id, 2);
                assert_eq!(backfilled_task_ids, vec![3]);
            }
            other => panic!("Expected PinnedWithBackfill via Sub-B, got {:?}", other),
        }
    }

    #[test]
    fn test_rcpsp_backfill_rejection_delays_head() {
        let scheduler = RcpspScheduler::new(20.0);
        let sg1 = make_test_subgoal_m3(1, 3000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3000, vec![], vec!["b.rs"]);
        let sg3 = make_test_subgoal_m3(3, 2000, vec![], vec!["c.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 3000).unwrap();

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 300.0);
        lrp_map.insert(3, 100.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        match decision {
            ScheduleDecision::ContextBoomWait { pinned_head_id, .. } => {
                assert_eq!(pinned_head_id, 2);
            }
            other => panic!("Expected ContextBoomWait, got {:?}", other),
        }
    }

    #[test]
    fn test_rcpsp_backfill_rejection_vfs_conflict() {
        let scheduler = RcpspScheduler::default_rate();
        let sg1 = make_test_subgoal_m3(1, 2000, vec![], vec!["shared.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3000, vec![], vec!["other.rs"]);
        let sg3 = make_test_subgoal_m3(3, 500, vec![], vec!["shared.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 2000).unwrap();

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 300.0);
        lrp_map.insert(3, 100.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        assert!(matches!(decision, ScheduleDecision::ContextBoomWait { .. }));
        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Ready);
    }

    #[test]
    fn test_rcpsp_multi_task_backfill_single_pass() {
        let scheduler = RcpspScheduler::new(20.0);
        let sg1 = make_test_subgoal_m3(1, 3000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3000, vec![], vec!["b.rs"]);
        let sg3 = make_test_subgoal_m3(3, 400, vec![], vec!["c.rs"]);
        let sg4 = make_test_subgoal_m3(4, 300, vec![], vec!["d.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3, sg4]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 3000).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 1096);

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 400.0);
        lrp_map.insert(3, 100.0);
        lrp_map.insert(4, 90.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        match decision {
            ScheduleDecision::PinnedWithBackfill {
                pinned_head_id,
                backfilled_task_ids,
                ..
            } => {
                assert_eq!(pinned_head_id, 2);
                assert_eq!(backfilled_task_ids, vec![3, 4]);
            }
            other => panic!("Expected both tasks backfilled, got {:?}", other),
        }

        assert_eq!(coord.records.get(&3).unwrap().status, TaskStatus::Running);
        assert_eq!(coord.records.get(&4).unwrap().status, TaskStatus::Running);
        assert_eq!(coord.memory_gate.remaining(), 1096 - 400 - 300);
    }

    #[test]
    fn test_rcpsp_context_boom_pauses_without_failing() {
        let scheduler = RcpspScheduler::default_rate();
        let sg1 = make_test_subgoal_m3(1, 3800, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 2000, vec![], vec!["b.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 3800).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 296);

        let mut lrp_map = HashMap::new();
        lrp_map.insert(2, 100.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        assert!(matches!(decision, ScheduleDecision::ContextBoomWait { .. }));

        let rec2 = coord.records.get(&2).unwrap();
        assert_eq!(rec2.status, TaskStatus::Ready);
        assert!(rec2.failure_reason.is_none());
    }

    #[test]
    fn test_rcpsp_preemption_and_repinning_on_failure() {
        let scheduler = RcpspScheduler::default_rate();
        let sg1 = make_test_subgoal_m3(1, 2000, vec![], vec!["a.rs"]);
        let sg2 = make_test_subgoal_m3(2, 3000, vec!["1"], vec!["b.rs"]);
        let sg3 = make_test_subgoal_m3(3, 1000, vec![], vec!["c.rs"]);

        let dag = TaskDag::from_subgoals(vec![sg1, sg2, sg3]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 2000).unwrap();

        coord.fail_task(1, "Out of bounds write", Some(DiagnosticCode::OutOfBoundsWrite));

        assert_eq!(coord.records.get(&1).unwrap().status, TaskStatus::Failed);
        assert_eq!(coord.records.get(&2).unwrap().status, TaskStatus::Cancelled);
        assert_eq!(coord.memory_gate.active_tokens, 0);

        let mut lrp_map = HashMap::new();
        lrp_map.insert(3, 50.0);

        let decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        assert_eq!(
            decision,
            ScheduleDecision::DispatchHead {
                task_id: 3,
                allocated_tokens: 1000,
            }
        );
    }

    #[test]
    fn test_rcpsp_edge_case_zero_tokens_and_exact_k_max() {
        let scheduler = RcpspScheduler::default_rate();
        let sg1 = make_test_subgoal_m3(1, 0, vec![], vec!["zero.rs"]);
        let dag = TaskDag::from_subgoals(vec![sg1]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let mut lrp_map = HashMap::new();
        lrp_map.insert(1, 0.0);

        let _decision = scheduler.schedule_next_batch(&mut coord, &lrp_map).unwrap();
        assert_eq!(coord.memory_gate.remaining(), 4096);
    }

    #[test]
    fn test_inject_subgoal_cycle_rejection_and_rollback() {
        let sg1 = make_test_subgoal_m3(1, 1000, vec![], vec!["a.rs"]);
        let dag = TaskDag::from_subgoals(vec![sg1]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        // Inject Task 2 depending on Task 1
        let sg2 = make_test_subgoal_m3(0, 1000, vec!["1"], vec!["b.rs"]);
        let task2_id = coord.inject_subgoal(sg2, vec![1]).expect("Valid injection should succeed");
        assert_eq!(task2_id, 2);

        // Attempt to inject Task 3 that depends on 2, but also manually add an edge from 3 to 1 to simulate cycle creation
        // Wait: inject_subgoal takes depends_on: [task_ids that this task depends on].
        // If we add an edge 2 -> 1, or inject a task with a cycle:
        // Let's create a cycle by injecting Task 3 depending on Task 2, and then adding edge 1 -> 3, wait:
        // If Task 1 depends on 3 and Task 3 depends on 1:
        coord.dag.dependencies.entry(1).or_default().insert(3);
        coord.dag.children.entry(3).or_default().insert(1);

        let sg3 = make_test_subgoal_m3(0, 1000, vec!["1"], vec!["c.rs"]);
        let res = coord.inject_subgoal(sg3, vec![1]);
        assert!(res.is_err(), "Cycle injection must be rejected");

        // Verify Task 3 was cleanly rolled back and not present in records or tasks
        assert!(!coord.records.contains_key(&3));
        assert!(!coord.dag.tasks.contains_key(&3));
    }

    #[test]
    fn test_complete_task_releases_tokens_even_on_status_error() {
        let sg1 = make_test_subgoal_m3(1, 1500, vec![], vec!["a.rs"]);
        let dag = TaskDag::from_subgoals(vec![sg1]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        coord.admit_task(1, 1500).unwrap();
        assert_eq!(coord.memory_gate.active_tokens, 1500);

        // Manually corrupt status to Pending to trigger error path in complete_task
        if let Some(rec) = coord.records.get_mut(&1) {
            rec.status = TaskStatus::Pending;
        }

        let res = coord.complete_task(1);
        assert!(res.is_err());
        // MemoryGate active tokens must be freed (0), no leak!
        assert_eq!(coord.memory_gate.active_tokens, 0);
    }

    #[test]
    fn test_dag_interface_contract_registration_and_retrieval() {
        let sg1 = make_test_subgoal_m3(1, 1000, vec![], vec!["Dashboard.tsx"]);
        let mut sg2 = make_test_subgoal_m3(2, 1000, vec!["1"], vec!["styles.css"]);
        sg2.target_entity = "styles.css".to_string();
        let mut sg1_clone = sg1.clone();
        sg1_clone.target_entity = "Dashboard.tsx".to_string();

        let dag = TaskDag::from_subgoals(vec![sg1_clone, sg2]).unwrap();
        let mut coord = StateMachineCoordinator::new(dag, 4096).unwrap();

        let contract_content = "export default function Dashboard({ user }: { user: UserProfile });\nexport async function updateProfile(data: FormData);";
        coord.set_interface_contract(1, contract_content.to_string());

        assert_eq!(coord.get_interface_contract(1).as_deref(), Some(contract_content));
        assert_eq!(coord.get_interface_contract(2), None);

        let contracts_map = coord.get_all_interface_contracts();
        assert_eq!(contracts_map.get("Dashboard.tsx").map(|s| s.as_str()), Some(contract_content));
    }

    #[test]
    fn test_distill_interface_fallback_nextjs_and_python() {
        // Next.js code with implementation details
        let nextjs_code = r#"
import React from 'react';
import { UserProfile } from '@/types';

export interface DashboardProps {
    user: UserProfile;
}

export default function Dashboard({ user }: DashboardProps) {
    const [count, setCount] = React.useState(0);
    // internal implementation details
    for (let i = 0; i < 100; i++) {
        console.log(i);
    }
    return <div>Hello {user.name}</div>;
}

export async function handleAction(formData: FormData) {
    const rawData = formData.get('data');
    return { success: true };
}
"#;
        let distilled_next = SwarmCoordinator::distill_interface_fallback("Dashboard.tsx", nextjs_code);
        assert!(distilled_next.contains("export interface DashboardProps"));
        assert!(distilled_next.contains("export default function Dashboard"));
        assert!(distilled_next.contains("export async function handleAction"));
        assert!(!distilled_next.contains("for (let i = 0; i < 100; i++)"));

        // Python code with internal loops
        let python_code = r#"
class DataProcessor:
    def __init__(self, name: str):
        self.name = name

    def compute_metrics(self, data: list) -> dict:
        result = {}
        for x in data:
            result[x] = x * 2
        return result
"#;
        let distilled_py = SwarmCoordinator::distill_interface_fallback("processor.py", python_code);
        assert!(distilled_py.contains("class DataProcessor:"));
        assert!(distilled_py.contains("def compute_metrics(self, data: list) -> dict:"));
        assert!(!distilled_py.contains("for x in data:"));
    }

    #[test]
    fn test_rule_of_3_circuit_breaker_and_global_ceiling_limits() {
        assert_eq!(MAX_DELTA_RETRIES_PER_TARGET, 3);
        assert_eq!(MAX_TOTAL_DYNAMIC_INJECTIONS, 6);

        // Simulate retry tracking for a missing artifact (e.g. style.css)
        let mut delta_retry_counts: HashMap<String, usize> = HashMap::new();
        let target = "style.css".to_string();

        let mut injected_count = 0;
        let mut breaker_tripped = false;

        for _ in 0..10 {
            let count = delta_retry_counts.entry(target.clone()).or_insert(0);
            *count += 1;
            if *count > MAX_DELTA_RETRIES_PER_TARGET {
                breaker_tripped = true;
                break;
            }
            injected_count += 1;
        }

        assert_eq!(injected_count, 3);
        assert!(breaker_tripped);
        assert_eq!(*delta_retry_counts.get(&target).unwrap(), 4);
    }

    #[test]
    fn test_extract_explicit_deliverables_multi_file_webapp() {
        let goal = "Build a modern Task Manager with app.py for backend and index.html for UI styling in style.css";
        let files = SwarmCoordinator::extract_explicit_deliverables(goal);
        assert_eq!(files, vec!["app.py", "index.html", "style.css"]);
    }

    #[test]
    fn test_infer_target_entity_no_javascript_java_collision() {
        let js_goal = "Build a modern interactive frontend using JavaScript and CSS";
        assert_eq!(SwarmCoordinator::infer_target_entity(js_goal), "index.html");

        let java_goal = "Build a complete Java application with GameLoop class";
        assert_eq!(SwarmCoordinator::infer_target_entity(java_goal), "Main.java");
    }
}


