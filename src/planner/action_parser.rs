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
                    "Provide the complete implementation for '{}' inside a code block (e.g. ```python ... ``` or ```rust ... ```).",
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
                format!("File '{}' was not deleted. Execute removal command via terminal.", target_entity)
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

    /// Build the idiomatic non-root installation command for the detected package manager
    pub fn build_install_command(&self, dep: &str) -> String {
        let dep_lower = dep.to_lowercase();
        match dep_lower.as_str() {
            "rustc" | "cargo" | "rust" => {
                match self {
                    HostPackageManager::Dnf => "dnf install -y rust cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_string(),
                    HostPackageManager::Apt => "apt-get install -y rustc cargo || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_string(),
                    HostPackageManager::Pacman => "pacman -S --noconfirm rust cargo".to_string(),
                    HostPackageManager::Brew => "brew install rust".to_string(),
                    _ => "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y".to_string(),
                }
            }
            "gcc" | "g++" | "c" | "cpp" | "c++" | "make" => {
                match self {
                    HostPackageManager::Dnf => "dnf install -y gcc gcc-c++ make".to_string(),
                    HostPackageManager::Apt => "apt-get install -y build-essential".to_string(),
                    HostPackageManager::Pacman => "pacman -S --noconfirm base-devel gcc".to_string(),
                    HostPackageManager::Brew => "brew install gcc make".to_string(),
                    _ => "gcc --version || clang --version".to_string(),
                }
            }
            "node" | "npm" | "npx" | "javascript" | "js" | "typescript" | "ts" => {
                match self {
                    HostPackageManager::Dnf => "dnf install -y nodejs npm".to_string(),
                    HostPackageManager::Apt => "apt-get install -y nodejs npm".to_string(),
                    HostPackageManager::Pacman => "pacman -S --noconfirm nodejs npm".to_string(),
                    HostPackageManager::Brew => "brew install node".to_string(),
                    _ => "npm install --global".to_string(),
                }
            }
            "javac" | "java" | "jdk" => {
                match self {
                    HostPackageManager::Dnf => "dnf install -y java-latest-openjdk-devel || dnf install -y java-17-openjdk-devel".to_string(),
                    HostPackageManager::Apt => "apt-get install -y default-jdk".to_string(),
                    HostPackageManager::Pacman => "pacman -S --noconfirm jdk-openjdk".to_string(),
                    HostPackageManager::Brew => "brew install openjdk".to_string(),
                    _ => "java -version".to_string(),
                }
            }
            "go" | "golang" => {
                match self {
                    HostPackageManager::Dnf => "dnf install -y golang".to_string(),
                    HostPackageManager::Apt => "apt-get install -y golang-go".to_string(),
                    HostPackageManager::Pacman => "pacman -S --noconfirm go".to_string(),
                    HostPackageManager::Brew => "brew install go".to_string(),
                    _ => "go version".to_string(),
                }
            }
            _ => {
                match self {
                    HostPackageManager::Brew => {
                        format!("brew install {} || python3 -m pip install --user {}", dep, dep)
                    }
                    HostPackageManager::Winget => {
                        format!("python -m pip install --user {} || pip install --user {}", dep, dep)
                    }
                    _ => {
                        format!(
                            "python3 -m pip install --user {} || pip install --user {} || pip install {}",
                            dep, dep, dep
                        )
                    }
                }
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

/// Strip prefixes like RUN:, EXEC:, TERMINAL:, $, #, -, etc. from an execution command
pub fn strip_command_prefix(raw: &str) -> String {
    let mut s = raw.trim();
    loop {
        let mut changed = false;
        if let Some(rest) = s.strip_prefix("Assistant:") {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("Action:").or_else(|| s.strip_prefix("action:")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("Command:").or_else(|| s.strip_prefix("command:")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("RUN:").or_else(|| s.strip_prefix("run:")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("EXEC:").or_else(|| s.strip_prefix("exec:")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("TERMINAL:").or_else(|| s.strip_prefix("terminal:")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix('$').or_else(|| s.strip_prefix('#')) {
            s = rest.trim();
            changed = true;
        }
        if let Some(rest) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")) {
            s = rest.trim();
            changed = true;
        }
        if let Some(dot_pos) = s.find(". ") {
            if dot_pos > 0 && s[..dot_pos].chars().all(|c| c.is_ascii_digit()) {
                s = s[dot_pos + 2..].trim();
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    s.to_string()
}

/// Parse raw text generated by the worker model into typed SwarmAction
pub fn parse_action(text: &str) -> SwarmAction {
    let stripped = strip_thought(text);
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
                if upper.starts_with("STATUS:") || upper.starts_with("DECISION:") {
                    let val = trimmed.splitn(2, ':').nth(1).unwrap_or("").trim().to_uppercase();
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
            let clean = strip_command_prefix(block);
            return SwarmAction::RunTerminal { command: clean };
        } else if !block.is_empty() {
            return SwarmAction::WriteFile { filename: String::new(), content: block.to_string() };
        }
    }

    // 3. Language-agnostic raw code detection without markdown fences
    if cleaned.starts_with("#!/")
        || cleaned.starts_with("import ")
        || cleaned.starts_with("from ")
        || cleaned.starts_with("def ")
        || cleaned.starts_with("class ")
        || cleaned.starts_with("use ")
        || cleaned.starts_with("pub ")
        || cleaned.starts_with("fn ")
        || cleaned.starts_with("struct ")
        || cleaned.starts_with("enum ")
        || cleaned.starts_with("impl ")
        || cleaned.starts_with("#include ")
        || cleaned.starts_with("package ")
        || cleaned.starts_with("func ")
        || cleaned.starts_with("<!DOCTYPE")
        || cleaned.starts_with("<html")
    {
        return SwarmAction::WriteFile { filename: String::new(), content: cleaned.to_string() };
    }

    // 4. Standard commands line by line
    for line in cleaned.lines() {
        let mut l = line.trim();
        if let Some(rest) = l.strip_prefix("Assistant:") {
            l = rest.trim();
        }

        if let Some(rest) = l.strip_prefix("- ")
            .or_else(|| l.strip_prefix("* "))
            .or_else(|| l.strip_prefix("1. "))
            .or_else(|| l.strip_prefix("2. "))
        {
            l = rest.trim();
        }

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
            let clean = strip_command_prefix(l);
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

pub fn parse_thinker_decision(text: &str, target_entity: &str) -> ThinkerDecision {
    let clean = strip_thought(text);
    let upper = clean.to_uppercase();
    if upper.contains("DECISION: RESET") || upper.contains("DECISION:RESET") {
        let reason = if let Some(idx) = clean.find("REASON:") {
            clean[idx + 7..].lines().next().unwrap_or("").trim().to_string()
        } else if let Some(idx) = clean.find('|') {
            clean[idx + 1..].lines().next().unwrap_or("").trim().to_string()
        } else {
            format!("Rewrite complete implementation for {}", target_entity)
        };
        ThinkerDecision::Reset(reason)
    } else {
        let reason = if let Some(idx) = clean.find("REASON:") {
            clean[idx + 7..].lines().next().unwrap_or("").trim().to_string()
        } else if let Some(idx) = clean.find('|') {
            clean[idx + 1..].lines().next().unwrap_or("").trim().to_string()
        } else {
            format!("Fix syntax error and ensure {} is written to VFS", target_entity)
        };
        ThinkerDecision::Nudge(reason)
    }
}

pub fn extract_missing_dependency(text: &str) -> Option<String> {
    const ENGINE_INTERNAL_KEYWORDS: &[&str] = &[
        "vfs", "sandbox", "cleanroom", "bwrap", "omni", "omni_engine", "workspace", "stdout", "stderr", "sys", "os",
        "abc", "argparse", "array", "ast", "asyncio", "base64", "binascii", "builtins", "bz2",
        "calendar", "cmath", "collections", "colorsys", "compileall", "concurrent", "configparser",
        "contextlib", "copy", "csv", "ctypes", "dataclasses", "datetime", "dbm", "difflib",
        "dis", "doctest", "email", "encodings", "enum", "errno", "faulthandler", "fcntl",
        "filecmp", "fileinput", "fnmatch", "fractions", "functools", "gc", "getopt", "getpass",
        "gettext", "glob", "graphlib", "gzip", "hashlib", "heapq", "hmac", "html", "http",
        "idlelib", "imaplib", "imghdr", "importlib", "inspect", "io", "ipaddress", "itertools",
        "json", "keyword", "linecache", "locale", "logging", "lzma", "mailbox", "mailcap",
        "marshal", "math", "mimetypes", "mmap", "modulefinder", "multiprocessing", "netrc",
        "nntplib", "numbers", "operator", "optparse", "pathlib", "pdb", "pickle",
        "pickletools", "pipes", "pkgutil", "platform", "plistlib", "poplib", "posix",
        "posixpath", "pprint", "profile", "pstats", "pty", "pwd", "py_compile", "pyclbr",
        "pydoc", "queue", "quopri", "random", "re", "reprlib", "resource", "rlcompleter",
        "runpy", "sched", "secrets", "select", "selectors", "shelve", "shlex", "shutil",
        "signal", "site", "smtpd", "smtplib", "sndhdr", "socket", "socketserver", "spwd",
        "sqlite3", "sre_compile", "sre_constants", "sre_parse", "ssl", "stat", "statistics",
        "string", "stringprep", "struct", "subprocess", "sunau", "symbol", "symtable",
        "sysconfig", "syslog", "tabnanny", "tarfile", "telnetlib", "tempfile",
        "termios", "test", "tests", "textwrap", "threading", "time", "timeit",
        "token", "tokenize", "tomllib", "trace", "traceback", "tracemalloc", "tty",
        "turtle", "turtledemo", "types", "typing", "unicodedata", "unittest", "urllib",
        "uu", "uuid", "venv", "warnings", "wave", "weakref", "webbrowser", "wsgiref",
        "xdrlib", "xml", "xmlrpc", "zipapp", "zipfile", "zipimport", "zlib", "zoneinfo",
        "src", "app", "lib", "pkg", "core", "models", "utils", "views", "controllers",
        "routes", "services", "helpers", "database", "cli", "library", "client", "server",
        "fixtures", "common", "shared", "internal", "config", "constants",
    ];

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
            if let Some(cand) = candidate {
                let clean = cand.trim_matches('\'').trim_matches('"').trim_matches(':');
                if !clean.is_empty()
                    && !IGNORED_COMMANDS.contains(&clean.to_lowercase().as_str())
                    && !ENGINE_INTERNAL_KEYWORDS.contains(&clean.to_lowercase().as_str())
                {
                    return Some(clean.to_string());
                }
            }
        }
    }

    None
}
