use crate::planner::system_profile::ToolCapability;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionIntent {
    pub is_execution_task: bool,
    pub required_tools: Vec<ToolCapability>,
    pub core_intent: String,
    pub detected_targets: Vec<String>,
}

/// Epistemic Pre-Pass Triage (Pass 1).
/// Evaluates user inquiry structure in an isolated ephemeral execution pass.
/// Extracts required toolsets and intent while guaranteeing ZERO context residue
/// and ZERO pollution in the primary Causal DAG.
pub struct PrePassTriage;

impl PrePassTriage {
    pub fn evaluate(query: &str) -> ExecutionIntent {
        let q = query.trim().to_lowercase();

        let mut required_tools = Vec::new();
        let mut is_execution_task = false;
        let mut detected_targets = Vec::new();

        // 1. Web / News / Online signals
        let web_signals = [
            "web", "internet", "news", "online", "search", "fetch", "download", "google",
            "wikipedia", "articles", "نت", "انترنت", "أخبار", "اخبار", "مقال", "بحث", "تصفح"
        ];
        if web_signals.iter().any(|s| q.contains(s)) {
            required_tools.push(ToolCapability::WebBrowser);
            is_execution_task = true;
        }

        // 2. Terminal / OS / Game launcher / Script execution signals
        let terminal_signals = [
            "terminal", "exec", "run", "launch", "launcher", "minecraft", "install", "build",
            "compile", "cargo", "rust", "python", "bash", "shell", "apt", "script",
            "ترمنال", "طرفية", "شغل", "شغّل", "لانشر", "ماين كرافت", "تثبيت", "برمجة", "كومبايل"
        ];
        if terminal_signals.iter().any(|s| q.contains(s)) {
            required_tools.push(ToolCapability::TerminalBridge);
            is_execution_task = true;
        }

        // 3. File / Storage / Document generation signals
        let file_signals = [
            "file", "txt", "csv", "json", "doc", "save", "write", "create file", "export",
            "ملف", "اكتب في", "حفظ", "انشئ ملف", "تصدير"
        ];
        if file_signals.iter().any(|s| q.contains(s)) || is_execution_task {
            if !required_tools.contains(&ToolCapability::MemoryVfs) {
                required_tools.push(ToolCapability::MemoryVfs);
            }
            is_execution_task = true;
        }

        // Target filenames
        for word in q.split_whitespace() {
            let clean = word.trim_matches(|c| c == '\'' || c == '"' || c == ',' || c == '.');
            if clean.ends_with(".txt") || clean.ends_with(".csv") || clean.ends_with(".json") || clean.ends_with(".rs") || clean.ends_with(".py") {
                detected_targets.push(clean.to_string());
            }
        }

        ExecutionIntent {
            is_execution_task,
            required_tools,
            core_intent: query.trim().to_string(),
            detected_targets,
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
    }
}
