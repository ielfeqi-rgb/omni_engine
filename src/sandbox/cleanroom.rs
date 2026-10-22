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

use std::path::PathBuf;
use std::time::Duration;
use crate::sandbox::isolated_jail::{IsolatedJail, JailExecutionResult};
use crate::sandbox::diagnostics::TopologicalDiagnostic;
use crate::planner::swarm_coordinator::SwarmCoordinator;

/// Mode A: Detailed Semantic and Structural Analysis Report for Written Content / Documents
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WrittenAnalysisReport {
    pub is_complete: bool,
    pub score: f32,
    pub word_count: usize,
    pub placeholder_count: usize,
    pub sections_detected: Vec<String>,
    pub detected_format: String,
    pub covered_topics: Vec<String>,
    pub missing_topics: Vec<String>,
    pub critique: String,
}

/// Mode B: Detailed Compiler, AST & Line-by-Line Syntax Analysis Report for Source Code
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CodeAnalysisReport {
    pub is_valid: bool,
    pub exit_code: Option<i32>,
    pub error_line: Option<usize>,
    pub error_message: Option<String>,
    pub broken_snippet: Option<String>,
    pub raw_output: String,
    pub verification_command: String,
}

/// Mode C: Interactive Execution and Live Dialogue Test Report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InteractiveExecutionReport {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub full_stdout: String,
    pub full_stderr: String,
    pub interactions_count: usize,
    pub timed_out: bool,
}

/// Mode D: Acceptance Criteria and Goal-Completeness Delta Report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AcceptanceDeltaReport {
    pub is_accepted: bool,
    pub target_file: String,
    pub covered_requirements: Vec<String>,
    pub missing_requirements: Vec<String>,
    pub delta_summary: String,
    pub recommendation: String,
}

/// Mode E: Cognitive Test Sheet Protocol Specification (ورقة الاختبار)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, PartialEq)]
pub struct TestSheet {
    pub required_files: Vec<String>,
    pub semantic_markers: Vec<String>,
    pub interactive_inputs: Vec<String>,
    pub expected_outputs: Vec<String>,
    pub verification_command: Option<String>,
}

impl TestSheet {
    /// Creates a default, unconstrained TestSheet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dynamically constructs a Cognitive Test Sheet using planned deliverables determined
    /// directly by the System 2 Sovereign Architect / Planner, while unconditionally preserving
    /// all deliverables explicitly mandated by the user in user_goal (Supreme Authority).
    pub fn from_planned_files(user_goal: &str, planned_files: &[String]) -> Self {
        let mut sheet = Self::parse_from_goal(user_goal);
        for file in planned_files {
            let trimmed = file.trim().to_string();
            if !trimmed.is_empty() && trimmed != "none" {
                if !sheet.required_files.iter().any(|f| f.eq_ignore_ascii_case(&trimmed)) {
                    sheet.required_files.push(trimmed);
                }
            }
        }
        sheet
    }

    /// Dynamically parses and synthesizes a Cognitive Test Sheet directly from user objective.
    /// Language-agnostic: dynamically recognizes valid filenames (*.ext, Dockerfile, Makefile)
    /// without relying on a static hardcoded extension whitelist.
    pub fn parse_from_goal(user_goal: &str) -> Self {
        let mut required_files = Vec::new();
        let mut semantic_markers = Vec::new();
        let mut interactive_inputs = Vec::new();
        let expected_outputs = Vec::new();

        let lower_goal = user_goal.to_lowercase();

        // 1. Language-agnostic extraction of explicit file targets mentioned in user_goal.
        // Validates file tokens (stem.ext) dynamically while avoiding numeric decimals (e.g. 3.14, 2.5) or domains.
        for word in user_goal.split_whitespace() {
            let clean = word.trim_matches(|c: char| {
                c == '\'' || c == '"' || c == '`' || c == ',' || c == '.' || c == ';'
                    || c == '(' || c == ')' || c == '[' || c == ']' || c == '{' || c == '}'
                    || c == ':' || c == '<' || c == '>' || c == '!' || c == '?'
            });
            if clean.contains('.') {
                let parts: Vec<&str> = clean.split('.').collect();
                if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
                    let stem = parts[0];
                    let ext = parts[1];
                    let is_numeric = ext.chars().all(|c| c.is_ascii_digit());
                    let is_valid_stem = stem.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
                    let is_valid_ext = ext.chars().all(|c| c.is_alphanumeric()) && !is_numeric && ext.len() <= 10;
                    let is_common_web_domain = matches!(ext.to_lowercase().as_str(), "com" | "org" | "net" | "edu" | "gov");
                    if is_valid_stem && is_valid_ext && !is_common_web_domain {
                        let filename = clean.to_string();
                        if !required_files.iter().any(|f: &String| f.eq_ignore_ascii_case(&filename)) {
                            required_files.push(filename);
                        }
                    }
                }
            } else if clean.eq_ignore_ascii_case("dockerfile") || clean.eq_ignore_ascii_case("makefile") {
                let filename = clean.to_string();
                if !required_files.iter().any(|f: &String| f.eq_ignore_ascii_case(&filename)) {
                    required_files.push(filename);
                }
            }
        }

        // 2. Extract semantic markers (Web, styles, keyframe animations, UI components)
        if lower_goal.contains("glassmorphism") {
            semantic_markers.push("glassmorphism".to_string());
            semantic_markers.push("backdrop-filter".to_string());
        }
        if lower_goal.contains("keyframe") || lower_goal.contains("animation") {
            semantic_markers.push("keyframes".to_string());
        }
        if lower_goal.contains("gradient") {
            semantic_markers.push("gradient".to_string());
        }
        if lower_goal.contains("glow") || lower_goal.contains("glowing") {
            semantic_markers.push("box-shadow".to_string());
        }
        if lower_goal.contains("responsive") {
            semantic_markers.push("@media".to_string());
        }

        // 3. Extract interactive test inputs if prompt represents an interactive application
        if lower_goal.contains("interactive") || lower_goal.contains("cli loop") || lower_goal.contains("menu") || lower_goal.contains("calculator") {
            if lower_goal.contains("calculator") {
                interactive_inputs.push("1".to_string());
                interactive_inputs.push("5".to_string());
                interactive_inputs.push("exit".to_string());
            } else {
                interactive_inputs.push("1".to_string());
                interactive_inputs.push("test_input".to_string());
                interactive_inputs.push("exit".to_string());
            }
        }

        Self {
            required_files,
            semantic_markers,
            interactive_inputs,
            expected_outputs,
            verification_command: None,
        }
    }
}

/// Universal Dedicated Cleanroom for Testing & Auditing:
/// 1. Semantic and Structural Auditing of Written Content / Docs / Specifications.
/// 2. Language-Agnostic Code & Line-by-Line AST Analysis with Micro-Surgeon extraction.
/// 3. Real Interactive Execution with synthetic/mock STDIN feeds and dialogue verification.
/// 4. Acceptance Criteria Auditing against multi-part user goals.
pub struct UniversalCleanroom;

impl UniversalCleanroom {
    /// Mode 1: Audit Written Artifacts (Markdown, JSON, CSV, Text Specifications, Docs)
    pub fn audit_written_content(content: &str, required_topics: &[&str]) -> WrittenAnalysisReport {
        let trimmed = content.trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let word_count = words.len();

        // 1. Detect format
        let detected_format = if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            "JSON".to_string()
        } else if trimmed.lines().any(|l| l.starts_with('#')) {
            "Markdown".to_string()
        } else if trimmed.lines().count() > 1 && trimmed.lines().next().map(|l| l.contains(',')).unwrap_or(false) {
            "CSV".to_string()
        } else {
            "PlainText".to_string()
        };

        // 2. Detect sections / headings
        let mut sections_detected = Vec::new();
        for line in trimmed.lines() {
            let l = line.trim();
            if l.starts_with('#') {
                sections_detected.push(l.trim_start_matches('#').trim().to_string());
            }
        }

        // 3. Detect placeholders
        let placeholder_patterns = [
            "TODO", "FIXME", "XXX", "TBD", "LOREM IPSUM", "INSERT CODE HERE",
            "YOUR CODE HERE", "UNIMPLEMENTED", "PASS", "...", "[INSERT", "<PLACEHOLDER>",
        ];
        let upper = trimmed.to_uppercase();
        let mut placeholder_count = 0;
        for pat in &placeholder_patterns {
            placeholder_count += upper.matches(pat).count();
        }

        // 4. Topic coverage analysis
        let mut covered_topics = Vec::new();
        let mut missing_topics = Vec::new();
        for topic in required_topics {
            let top_lower = topic.to_lowercase();
            if trimmed.to_lowercase().contains(&top_lower) {
                covered_topics.push(topic.to_string());
            } else {
                missing_topics.push(topic.to_string());
            }
        }

        // 5. Scoring calculation
        let mut score: f32 = 1.0;
        if word_count < 20 {
            score -= 0.5;
        } else if word_count < 50 {
            score -= 0.2;
        }

        if placeholder_count > 0 {
            score -= (placeholder_count as f32 * 0.15).min(0.6);
        }

        if !required_topics.is_empty() {
            let topic_coverage = covered_topics.len() as f32 / required_topics.len() as f32;
            score = (score * 0.5) + (topic_coverage * 0.5);
        }

        score = score.clamp(0.0, 1.0);
        let is_complete = placeholder_count == 0 && score >= 0.7 && missing_topics.is_empty();

        let mut critique = String::new();
        if !is_complete {
            if placeholder_count > 0 {
                critique.push_str(&format!("Found {} unresolved placeholder(s). ", placeholder_count));
            }
            if !missing_topics.is_empty() {
                critique.push_str(&format!("Missing required topics: {:?}. ", missing_topics));
            }
            if word_count < 20 {
                critique.push_str("Content is too brief or incomplete. ");
            }
        } else {
            critique = "Document satisfies acceptance criteria cleanly.".to_string();
        }

        WrittenAnalysisReport {
            is_complete,
            score,
            word_count,
            placeholder_count,
            sections_detected,
            detected_format,
            covered_topics,
            missing_topics,
            critique,
        }
    }

    /// Mode 2: Language-Agnostic Code & Line-by-Line AST Analysis
    pub fn audit_code_syntax(filename: &str, code_bytes: &[u8]) -> CodeAnalysisReport {
        Self::audit_code_syntax_with_vfs(filename, code_bytes, &[])
    }

    /// Mode 2: Language-Agnostic Code & Line-by-Line AST Analysis with project VFS context
    pub fn audit_code_syntax_with_vfs(
        filename: &str,
        code_bytes: &[u8],
        vfs_files: &[(PathBuf, Vec<u8>)],
    ) -> CodeAnalysisReport {
        let p = std::path::Path::new(filename);
        let base_name = p.file_name().and_then(|f| f.to_str()).unwrap_or(filename);
        let verification_cmd = SwarmCoordinator::resolve_syntax_check_command(base_name);

        let mut staged_files = Vec::new();
        // 1. Stage the file being audited
        staged_files.push((PathBuf::from(base_name), code_bytes.to_vec()));
        // 2. Stage all other existing project files from VFS so peer imports resolve cleanly
        for (rel_path, content) in vfs_files {
            let rel_base = rel_path.file_name().and_then(|f| f.to_str()).unwrap_or_else(|| rel_path.to_str().unwrap_or(""));
            if rel_base != base_name && !rel_base.is_empty() {
                staged_files.push((PathBuf::from(rel_base), content.clone()));
            }
        }

        let jail_res = match IsolatedJail::run_in_jail(&verification_cmd, &staged_files, Duration::from_secs(30)) {
            Ok(res) => res,
            Err(e) => JailExecutionResult {
                success: false,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("Cleanroom execution failure: {}", e),
                missing_dependency: None,
                sandboxed_by_bwrap: false,
            },
        };

        let mut combined = format!("{}\n{}", jail_res.stdout, jail_res.stderr);
        let mut overall_success = jail_res.success;
        let mut final_exit_code = jail_res.exit_code;

        // Semantic Runtime Smoke Testing for executable scripts
        if overall_success && base_name.ends_with(".py") {
            let smoke_cmd = format!("python3 {}", base_name);
            let simulated_inputs = "1\ntest task\n4\nexit\nquit\n5\n";
            if let Ok(smoke_res) = IsolatedJail::run_in_jail_with_input(
                &smoke_cmd,
                &staged_files,
                Duration::from_secs(10),
                Some(simulated_inputs.as_bytes()),
            ) {
                let smoke_combined = format!("{}\n{}", smoke_res.stdout, smoke_res.stderr);
                // An interactive script exhausting piped stdin throws EOFError when awaiting user input;
                // this is an interaction boundary, NOT a code syntax or runtime crash defect.
                let is_benign_eof = smoke_combined.contains("EOFError: EOF when reading a line")
                    || smoke_combined.contains("EOFError");

                let has_fatal_runtime_error = (smoke_combined.contains("Traceback (most recent call last)") && !is_benign_eof)
                    || smoke_combined.contains("NameError:")
                    || smoke_combined.contains("TypeError:")
                    || smoke_combined.contains("AttributeError:")
                    || smoke_combined.contains("UnboundLocalError:");

                if has_fatal_runtime_error {
                    overall_success = false;
                    final_exit_code = smoke_res.exit_code.or(Some(1));
                    combined = smoke_combined;
                }
            }
        }

        // Semantic UI & Web Contract Verification for frontend artifacts
        let is_ui_artifact = base_name.ends_with(".html")
            || base_name.ends_with(".js")
            || base_name.ends_with("test_steps.json");

        if overall_success && is_ui_artifact {
            // Find HTML content
            let html_bytes = staged_files
                .iter()
                .find(|(p, _)| p.to_string_lossy().ends_with(".html"))
                .map(|(_, b)| b.as_slice());

            if let Some(h_bytes) = html_bytes {
                let html_str = String::from_utf8_lossy(h_bytes);

                // Find external JS content
                let js_str = staged_files
                    .iter()
                    .find(|(p, _)| {
                        let s = p.to_string_lossy();
                        s.ends_with(".js") && !s.ends_with("tester.js") && !s.ends_with("inspector.js")
                    })
                    .map(|(_, b)| String::from_utf8_lossy(b).to_string());

                // Find test_steps.json
                let steps_str = staged_files
                    .iter()
                    .find(|(p, _)| p.to_string_lossy().ends_with("test_steps.json"))
                    .map(|(_, b)| String::from_utf8_lossy(b).to_string());

                // Find Python backend
                let py_str = staged_files
                    .iter()
                    .find(|(p, _)| {
                        let s = p.to_string_lossy();
                        s.ends_with(".py") && !s.contains("test_")
                    })
                    .map(|(_, b)| String::from_utf8_lossy(b).to_string());

                let report = crate::sandbox::ui_verifier::UiContractVerifier::verify_ui_artifact(
                    &html_str,
                    js_str.as_deref(),
                    steps_str.as_deref(),
                    py_str.as_deref(),
                );

                if !report.passed {
                    overall_success = false;
                    final_exit_code = Some(1);
                    combined = report.to_model_feedback();
                }
            }
        }

        let error_line = if !overall_success {
            SwarmCoordinator::extract_error_line_number(&combined, filename)
        } else {
            None
        };

        let code_str = String::from_utf8_lossy(code_bytes);
        let broken_snippet = if let Some(line) = error_line {
            TopologicalDiagnostic::extract_from_source(filename, &code_str, line)
                .map(|s| s.content)
                .ok()
        } else {
            None
        };

        let error_message = if !overall_success {
            let first_err_line = combined.lines()
                .find(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("error:") || lower.contains("syntaxerror") || lower.contains("nameerror:")
                        || lower.contains("typeerror:") || lower.contains("attributeerror:") || lower.contains("failed")
                })
                .unwrap_or_else(|| combined.lines().next().unwrap_or("Unknown compilation/runtime error"));
            Some(first_err_line.trim().to_string())
        } else {
            None
        };

        if overall_success {
            println!("  [CLEANROOM AST & SMOKE OK] '{}' passed compilation and execution gate", base_name);
        } else {
            println!("  [CLEANROOM FAILURE] '{}' failed verification (exit: {:?}): {}", base_name, final_exit_code, error_message.as_deref().unwrap_or("unknown error"));
        }

        CodeAnalysisReport {
            is_valid: overall_success,
            exit_code: final_exit_code,
            error_line,
            error_message,
            broken_snippet,
            raw_output: combined.trim().to_string(),
            verification_command: verification_cmd,
        }
    }

    /// Mode 3: Real Interactive Execution with synthetic/mock STDIN feeds
    pub fn execute_interactive(
        cmd: &str,
        staged_files: &[(PathBuf, Vec<u8>)],
        inputs: &[&str],
        timeout: Duration,
    ) -> InteractiveExecutionReport {
        let formatted_stdin = if !inputs.is_empty() {
            let mut buf = inputs.join("\n");
            buf.push('\n');
            Some(buf.into_bytes())
        } else {
            None
        };

        let jail_res = match IsolatedJail::run_in_jail_with_input(
            cmd,
            staged_files,
            timeout,
            formatted_stdin.as_deref(),
        ) {
            Ok(res) => res,
            Err(e) => JailExecutionResult {
                success: false,
                exit_code: Some(1),
                stdout: String::new(),
                stderr: format!("Cleanroom interactive failure: {}", e),
                missing_dependency: None,
                sandboxed_by_bwrap: false,
            },
        };

        let timed_out = jail_res.exit_code == Some(124);

        InteractiveExecutionReport {
            success: jail_res.success,
            exit_code: jail_res.exit_code,
            full_stdout: jail_res.stdout,
            full_stderr: jail_res.stderr,
            interactions_count: inputs.len(),
            timed_out,
        }
    }

    /// Mode 4: Acceptance Criteria Auditing against User Goal and Cognitive Test Sheet
    /// Strictly Read-Only (W = ∅) evaluation of whether staged code/artifacts fulfill the multi-part user goal.
    pub fn audit_acceptance_criteria_with_sheet(
        target_file: &str,
        code_or_doc: &str,
        user_goal: &str,
        test_sheet: &TestSheet,
        existing_vfs_files: &[String],
    ) -> AcceptanceDeltaReport {
        let trimmed = code_or_doc.trim();
        let lower_code = trimmed.to_lowercase();
        let lower_goal = user_goal.to_lowercase();

        let mut covered_requirements = Vec::new();
        let mut missing_requirements = Vec::new();

        // 1. Artifact Presence Check (Enforcing TestSheet required_files against VFS)
        for req_file in &test_sheet.required_files {
            let is_data_file = matches!(
                req_file.rsplit('.').next().map(|s| s.to_ascii_lowercase()).as_deref(),
                Some("json" | "txt" | "csv" | "log" | "sqlite" | "db")
            );
            let found = existing_vfs_files.iter().any(|v| {
                v.eq_ignore_ascii_case(req_file)
                    || v.ends_with(&format!("/{}", req_file))
                    || req_file.ends_with(v)
            }) || target_file.eq_ignore_ascii_case(req_file)
               || (is_data_file && lower_code.contains(&req_file.to_lowercase()));

            if found {
                covered_requirements.push(format!("Required artifact '{}' staged in VFS", req_file));
            } else {
                missing_requirements.push(format!("Missing required artifact '{}'", req_file));
            }
        }

        // 2. Semantic Markers Check (e.g. glassmorphism, backdrop-filter, keyframes)
        for marker in &test_sheet.semantic_markers {
            if lower_code.contains(marker) {
                covered_requirements.push(format!("Semantic marker '{}' implemented", marker));
            } else {
                missing_requirements.push(format!("Missing semantic marker '{}'", marker));
            }
        }

        // 3. Domain Heuristics from user goal (JSON, CLI loop, Calculator, History, Error handling)
        let mut expected_requirements: Vec<(&str, Vec<&str>)> = Vec::new();

        // JSON / File Persistence
        if lower_goal.contains("json") || lower_goal.contains("persist") || lower_goal.contains("save") || (lower_goal.contains("file") && !lower_goal.contains("single file")) {
            expected_requirements.push((
                "File/JSON persistence",
                vec!["json", "open(", "dump", "load", "write(", "read(", "with open", "fs.", "file", "tasks.json", "std::fs"]
            ));
        }

        // Interactive / CLI loop
        if lower_goal.contains("interactive") || lower_goal.contains("cli loop") || lower_goal.contains("menu") || lower_goal.contains("prompt") {
            expected_requirements.push((
                "Interactive CLI loop / user input",
                vec!["input(", "while ", "sys.stdin", "readline", "prompt", "menu", "stdin().read_line"]
            ));
        }

        // Error Handling / Corruption recovery
        if lower_goal.contains("corrupt") || lower_goal.contains("error") || lower_goal.contains("exception") || lower_goal.contains("graceful") {
            expected_requirements.push((
                "Error handling and corruption recovery",
                vec!["try:", "except ", "catch", "filenotfounderror", "jsondecodeerror", "result<", "match ", "err(", "io::error"]
            ));
        }

        // Deletion / Removal
        if lower_goal.contains("delete") || lower_goal.contains("remove") {
            expected_requirements.push((
                "Deletion / Removal capability",
                vec!["del ", ".remove(", ".pop(", "remove", "delete"]
            ));
        }

        // Calculation / Computation
        if lower_goal.contains("calculator") || lower_goal.contains("calc") || lower_goal.contains("compute") {
            expected_requirements.push((
                "Calculation / Arithmetic operations",
                vec!["+", "-", "*", "/", "add", "calculate", "result", "eval"]
            ));
        }

        // History / Logging
        if lower_goal.contains("history") || lower_goal.contains("log") {
            expected_requirements.push((
                "History logging / audit trail",
                vec!["history", "log", "append", "record", "entries"]
            ));
        }

        for (req_name, markers) in expected_requirements {
            let matched = markers.iter().any(|&m| lower_code.contains(m));
            if matched {
                covered_requirements.push(req_name.to_string());
            } else {
                missing_requirements.push(req_name.to_string());
            }
        }

        let is_accepted = missing_requirements.is_empty();
        let delta_summary = if is_accepted {
            "All identified acceptance criteria are present and verified in the deliverable.".to_string()
        } else {
            format!("Missing requirements: {}", missing_requirements.join("; "))
        };

        let recommendation = if is_accepted {
            "Ready for host commit.".to_string()
        } else {
            format!("Incrementally extend '{}' to implement: {}", target_file, missing_requirements.join("; "))
        };

        AcceptanceDeltaReport {
            is_accepted,
            target_file: target_file.to_string(),
            covered_requirements,
            missing_requirements,
            delta_summary,
            recommendation,
        }
    }

    /// Backwards-compatible convenience wrapper around `audit_acceptance_criteria_with_sheet`
    pub fn audit_acceptance_criteria(
        target_file: &str,
        code_or_doc: &str,
        user_goal: &str,
    ) -> AcceptanceDeltaReport {
        let test_sheet = TestSheet::from_planned_files(user_goal, &[target_file.to_string()]);
        let existing = vec![target_file.to_string()];
        Self::audit_acceptance_criteria_with_sheet(target_file, code_or_doc, user_goal, &test_sheet, &existing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sheet_parse_from_web_goal() {
        let goal = "Build a modern landing page in index.html and style.css with glassmorphism and keyframe animations";
        let sheet = TestSheet::parse_from_goal(goal);
        assert!(sheet.required_files.contains(&"index.html".to_string()));
        assert!(sheet.required_files.contains(&"style.css".to_string()));
        assert!(sheet.semantic_markers.contains(&"glassmorphism".to_string()));
        assert!(sheet.semantic_markers.contains(&"backdrop-filter".to_string()));
        assert!(sheet.semantic_markers.contains(&"keyframes".to_string()));
    }

    #[test]
    fn test_sheet_audit_detects_missing_file() {
        let goal = "Build a modern landing page in index.html and style.css with glassmorphism";
        let sheet = TestSheet::parse_from_goal(goal);
        let vfs_files = vec!["index.html".to_string()]; // style.css is MISSING like in Gemma-2B!
        let code = "<html><head><link rel='stylesheet' href='style.css'></head><body><h1>Hello</h1></body></html>";
        let report = UniversalCleanroom::audit_acceptance_criteria_with_sheet(
            "index.html",
            code,
            goal,
            &sheet,
            &vfs_files,
        );
        assert!(!report.is_accepted);
        assert!(report.missing_requirements.iter().any(|r| r.contains("style.css")));
    }

    #[test]
    fn test_sheet_audit_passes_when_all_files_and_markers_present() {
        let goal = "Build a landing page in index.html and style.css with glassmorphism";
        let sheet = TestSheet::parse_from_goal(goal);
        let vfs_files = vec!["index.html".to_string(), "style.css".to_string()];
        let code = "<html><div class='glassmorphism' style='backdrop-filter: blur(10px)'></div></html>";
        let report = UniversalCleanroom::audit_acceptance_criteria_with_sheet(
            "index.html",
            code,
            goal,
            &sheet,
            &vfs_files,
        );
        assert!(report.is_accepted);
        assert!(report.missing_requirements.is_empty());
    }

    #[test]
    fn test_sheet_interactive_inputs_parsing() {
        let goal = "Build an interactive CLI calculator menu in calculator.py";
        let sheet = TestSheet::parse_from_goal(goal);
        assert!(sheet.required_files.contains(&"calculator.py".to_string()));
        assert!(!sheet.interactive_inputs.is_empty());
        assert!(sheet.interactive_inputs.contains(&"exit".to_string()));
    }

    #[test]
    fn test_sheet_parse_arbitrary_extensions() {
        // Must recognize arbitrary extensions (.zig, .vue, .svelte, Dockerfile) without a static whitelist
        let goal = "Create microservices in main.zig, frontend in App.vue, and container in Dockerfile";
        let sheet = TestSheet::parse_from_goal(goal);
        assert!(sheet.required_files.contains(&"main.zig".to_string()));
        assert!(sheet.required_files.contains(&"App.vue".to_string()));
        assert!(sheet.required_files.contains(&"Dockerfile".to_string()));
    }

    #[test]
    fn test_sheet_from_planned_files() {
        // Derives deliverables directly from the planner even if not explicitly named in the prompt
        let goal = "Design an interactive weather dashboard with responsive glassmorphism";
        let planned = vec!["index.html".to_string(), "styles.css".to_string(), "app.js".to_string()];
        let sheet = TestSheet::from_planned_files(goal, &planned);
        assert_eq!(sheet.required_files.len(), 3);
        assert!(sheet.required_files.contains(&"index.html".to_string()));
        assert!(sheet.required_files.contains(&"styles.css".to_string()));
        assert!(sheet.required_files.contains(&"app.js".to_string()));
        assert!(sheet.semantic_markers.contains(&"glassmorphism".to_string()));
    }
}
