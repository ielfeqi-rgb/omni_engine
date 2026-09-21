use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReasoningMode {
    /// Fast / Direct execution for single-step answers, queries, trivial edits, or chat
    FastInteractive,
    /// Multi-turn goal agent loop with Speculative Branching, Sandbox verification, and Causal KV Rollback
    DeepAutonomous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageAssessment {
    pub selected_mode: ReasoningMode,
    pub confidence: f32,
    pub rationale: String,
    pub suggested_branches_count: usize,
}

/// Dynamic Thinking Mode Router.
/// Evaluates user requests and allows the model (or auto-classifier)
/// to select between FastInteractive and DeepAutonomous modes.
pub struct ModeRouter;

impl ModeRouter {
    /// Classify incoming inquiry into appropriate reasoning mode based on structural complexity signals
    pub fn assess_request(query: &str) -> TriageAssessment {
        let q = query.trim().to_lowercase();

        // 1. Explicit user overrides
        if q.starts_with("/fast") {
            return TriageAssessment {
                selected_mode: ReasoningMode::FastInteractive,
                confidence: 1.0,
                rationale: "User explicitly enforced /fast interactive mode".to_string(),
                suggested_branches_count: 1,
            };
        }

        if q.starts_with("/deep") || q.starts_with("/goal") {
            return TriageAssessment {
                selected_mode: ReasoningMode::DeepAutonomous,
                confidence: 1.0,
                rationale: "User explicitly enforced /deep autonomous mode".to_string(),
                suggested_branches_count: 3,
            };
        }

        // 2. Automated architectural heuristics (Autonomous Decision Gate)
        let deep_signals = [
            "ابني", "انشئ", "اعمل مشروع", "لعبه", "لعبة", "تطبيق", "معمارية", "تصميم كامل",
            "build", "create", "game", "project", "architecture", "refactor", "system",
            "خطوات", "مراحل", "فروع", "multi-step", "pipeline", "fps", "full-stack"
        ];

        let mut matched_signals = Vec::new();
        for signal in &deep_signals {
            if q.contains(signal) {
                matched_signals.push(*signal);
            }
        }

        let is_multi_word_goal = q.split_whitespace().count() >= 6;

        if !matched_signals.is_empty() && is_multi_word_goal {
            TriageAssessment {
                selected_mode: ReasoningMode::DeepAutonomous,
                confidence: 0.92,
                rationale: format!(
                    "Target detected as multi-stage goal requiring speculative branching (Matched signals: {:?})",
                    matched_signals
                ),
                suggested_branches_count: 3,
            }
        } else {
            TriageAssessment {
                selected_mode: ReasoningMode::FastInteractive,
                confidence: 0.88,
                rationale: "Direct request requiring fast interactive response without multi-branch speculation".to_string(),
                suggested_branches_count: 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_router_autonomous_decision() {
        // Test 1: Simple question should route to FastInteractive
        let fast_case = ModeRouter::assess_request("ما هو كرت الشاشة الأنسب للجهاز؟");
        assert_eq!(fast_case.selected_mode, ReasoningMode::FastInteractive);

        // Test 2: Complex project goal should autonomously route to DeepAutonomous
        let deep_case = ModeRouter::assess_request("عايزين نعمل لعبة FPS مصغرة في المتصفح مع حلقة تحكم بالماوس");
        assert_eq!(deep_case.selected_mode, ReasoningMode::DeepAutonomous);
        assert_eq!(deep_case.suggested_branches_count, 3);

        // Test 3: Explicit override
        let explicit_fast = ModeRouter::assess_request("/fast اعمل لعبه");
        assert_eq!(explicit_fast.selected_mode, ReasoningMode::FastInteractive);
    }
}
