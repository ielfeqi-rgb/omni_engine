use crate::planner::system_profile::ToolCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DualSystemPlan {
    pub is_execution_task: bool,
    pub required_tools: Vec<ToolCapability>,
    pub plan_a: String,
    pub plan_b: String,
    pub plan_c: String,
    pub caveman_constraints: Vec<String>,
    pub distilled_something_i_know: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionIntent {
    pub is_execution_task: bool,
    pub required_tools: Vec<ToolCapability>,
    pub core_intent: String,
    pub detected_targets: Vec<String>,
    pub dual_system_plan: Option<DualSystemPlan>,
}

/// Epistemic Pre-Pass Triage (Pass 1).
/// Evaluates user inquiry structure in an isolated ephemeral execution pass.
/// Extracts required toolsets, intent, and synthesizes System 2 Tri-Plans (A, B, C)
/// while guaranteeing ZERO context residue and ZERO pollution in the primary Causal DAG.
pub struct PrePassTriage;

impl PrePassTriage {
    pub fn evaluate(query: &str) -> ExecutionIntent {
        let q = query.trim().to_lowercase();

        let mut required_tools = Vec::new();
        let mut is_execution_task = false;
        let mut detected_targets = Vec::new();

        // 1. Web / News / Online / Information signals
        let web_signals = [
            "web", "internet", "news", "online", "search", "fetch", "download", "google",
            "wikipedia", "articles", "today", "latest", "events", "world",
            "نت", "انترنت", "أخبار", "اخبار", "مقال", "بحث", "تصفح", "احداث", "أحداث", "العالم", "جديد", "سوق"
        ];
        if web_signals.iter().any(|s| q.contains(s)) {
            required_tools.push(ToolCapability::WebBrowser);
            is_execution_task = true;
        }

        // 2. Terminal / OS / Game launcher / Script execution signals
        let terminal_signals = [
            "terminal", "exec", "run", "launch", "launcher", "minecraft", "install", "build",
            "compile", "cargo", "rust", "python", "bash", "shell", "apt", "script", "flatpak",
            "ترمنال", "طرفية", "شغل", "شغّل", "لانشر", "ماين كرافت", "تثبيت", "برمجة", "كومبايل", "نفذ", "نفّذ"
        ];
        if terminal_signals.iter().any(|s| q.contains(s)) {
            required_tools.push(ToolCapability::TerminalBridge);
            is_execution_task = true;
        }

        // Target filenames
        for word in q.split_whitespace() {
            let clean = word.trim_matches(|c| c == '\'' || c == '"' || c == ',' || c == '.');
            if clean.ends_with(".txt") || clean.ends_with(".csv") || clean.ends_with(".json") || clean.ends_with(".rs") || clean.ends_with(".py") || clean.ends_with(".sh") || clean.ends_with(".md") {
                detected_targets.push(clean.to_string());
            }
        }

        // 3. File / Storage / Document generation signals
        let file_signals = [
            "file", "txt", "csv", "json", "doc", "save", "write", "create file", "export", "notes", "log", "summary",
            "ملف", "اكتب في", "حفظ", "انشئ ملف", "تصدير", "تقرير", "سجل", "احفظ", "مستند", "ملخص"
        ];
        let has_explicit_file_intent = file_signals.iter().any(|s| q.contains(s)) || !detected_targets.is_empty();
        if has_explicit_file_intent || required_tools.contains(&ToolCapability::WebBrowser) {
            if !required_tools.contains(&ToolCapability::MemoryVfs) {
                required_tools.push(ToolCapability::MemoryVfs);
            }
            is_execution_task = true;
        }

        // System 2 Tri-Plan Synthesis
        let dual_system_plan = if is_execution_task {
            let target_str = if detected_targets.is_empty() {
                "output.txt".to_string()
            } else {
                detected_targets.join(", ")
            };

            let (plan_a, plan_b, plan_c, mut caveman_constraints) = if required_tools.contains(&ToolCapability::WebBrowser) {
                (
                    format!("browser.search(\"{}\") -> direct vfs.write(\"{}\", news)", query.trim(), target_str),
                    format!("browser.open(\"https://news.google.com\") -> vfs.write(\"{}\", ascii_summary)", target_str),
                    format!("web.fetch(\"https://news.google.com/rss\") -> strip_tags -> vfs.write(\"{}\", text)", target_str),
                    vec![
                        "no html regex or string.match on web text".to_string(),
                        "never index or get length of nil variable".to_string(),
                        "pipe browser output directly into vfs.write".to_string(),
                    ],
                )
            } else if required_tools.contains(&ToolCapability::TerminalBridge) && !required_tools.contains(&ToolCapability::MemoryVfs) {
                (
                    "terminal.exec(\"<command>\") -> execute direct host shell command".to_string(),
                    "terminal.exec(\"<fallback_command>\") -> alternative flag or package".to_string(),
                    "terminal.logs(20) -> inspect failure logs and diagnose".to_string(),
                    vec![
                        "terminal only: do NOT write files or use vfs".to_string(),
                        "linux native only, no windows pe .exe".to_string(),
                        "safe commands only, check exit codes with terminal.status".to_string(),
                    ],
                )
            } else if required_tools.contains(&ToolCapability::TerminalBridge) {
                (
                    "terminal.exec(\"which <cmd> || flatpak list\") -> verify existing binary".to_string(),
                    "terminal.exec(\"install command or portable script\")".to_string(),
                    format!("vfs.write(\"{}\", \"#!/usr/bin/env bash\\n...\") -> synthesize launcher script", target_str),
                    vec![
                        "linux native only, no windows pe .exe".to_string(),
                        "safe commands only, check exit codes".to_string(),
                    ],
                )
            } else {
                (
                    format!("vfs.write(\"{}\", content) -> pure direct write", target_str),
                    format!("vfs.write(\"{}\", fallback_content) -> minimal structure", target_str),
                    "terminal.exec(\"echo '...' > output\") -> host shell fallback".to_string(),
                    vec![
                        "stage in memory vfs before commit".to_string(),
                    ],
                )
            };

            caveman_constraints.push("output executable lua block ```lua ... ``` only".to_string());

            let mut blueprint = Vec::new();
            blueprint.push("[SOMETHING I KNOW / SYSTEM 2 BLUEPRINT]:".to_string());
            blueprint.push(format!("* TARGET: {}", target_str));
            blueprint.push(format!("* PLAN A (Primary): {}", plan_a));
            blueprint.push(format!("* PLAN B (Fallback): {}", plan_b));
            blueprint.push(format!("* PLAN C (Last Resort): {}", plan_c));
            blueprint.push(format!("* CAVEMAN INVARIANTS: {}", caveman_constraints.join("; ")));

            Some(DualSystemPlan {
                is_execution_task: true,
                required_tools: required_tools.clone(),
                plan_a,
                plan_b,
                plan_c,
                caveman_constraints,
                distilled_something_i_know: blueprint.join("\n"),
            })
        } else {
            None
        };

        ExecutionIntent {
            is_execution_task,
            required_tools,
            core_intent: query.trim().to_string(),
            detected_targets,
            dual_system_plan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_pass_triage_web_news() {
        let intent = PrePassTriage::evaluate("Fetch today's top news summaries from internet and save to news.txt");
        assert!(intent.is_execution_task);
        assert!(intent.required_tools.contains(&ToolCapability::WebBrowser));
        assert!(intent.required_tools.contains(&ToolCapability::MemoryVfs));
        assert!(intent.detected_targets.contains(&"news.txt".to_string()));
    }

    #[test]
    fn test_pre_pass_triage_minecraft_launcher() {
        let intent = PrePassTriage::evaluate("I want to play Minecraft for free on Linux, build a launcher or install it");
        assert!(intent.is_execution_task);
        assert!(intent.required_tools.contains(&ToolCapability::TerminalBridge));
    }

    #[test]
    fn test_pre_pass_triage_pure_chat() {
        let intent = PrePassTriage::evaluate("How does quantum gravity work?");
        assert!(!intent.is_execution_task);
        assert!(intent.required_tools.is_empty());
        assert!(intent.dual_system_plan.is_none());
    }

    #[test]
    fn test_dual_system_tri_plan_synthesis() {
        let intent = PrePassTriage::evaluate("Download news from internet and save to /home/hema/today_news.txt");
        assert!(intent.is_execution_task);
        let plan = intent.dual_system_plan.expect("DualSystemPlan should be generated");
        assert!(plan.plan_a.contains("browser.search"));
        assert!(plan.plan_b.contains("browser.open"));
        assert!(plan.plan_c.contains("web.fetch"));
        assert!(plan.caveman_constraints.iter().any(|c| c.contains("no html regex")));
        assert!(plan.distilled_something_i_know.contains("[SOMETHING I KNOW"));
    }

    #[test]
    fn test_pre_pass_triage_terminal_only() {
        let intent = PrePassTriage::evaluate("Check cargo check and compile code in terminal");
        assert!(intent.is_execution_task);
        assert_eq!(intent.required_tools, vec![ToolCapability::TerminalBridge]);
        let plan = intent.dual_system_plan.expect("DualSystemPlan should exist");
        assert!(plan.plan_a.contains("terminal.exec"));
        assert!(plan.caveman_constraints.iter().any(|c| c.contains("terminal only")));
    }
}
