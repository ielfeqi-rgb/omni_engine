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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCapability {
    WebBrowser,
    TerminalBridge,
    MemoryVfs,
}

/// Dynamic System Profile & Grounding Engine.
/// Generates tailored prompts matching exact tool capabilities required for a goal,
/// eliminating attention dilution and tool hallucinations on compact models.
pub struct GroundedSystemProfile;

impl GroundedSystemProfile {
    pub fn build_system_prompt() -> String {
        Self::build_tailored_prompt(&[
            ToolCapability::WebBrowser,
            ToolCapability::TerminalBridge,
            ToolCapability::MemoryVfs,
        ])
    }

    pub fn build_tailored_prompt(tools: &[ToolCapability]) -> String {
        let os_name = if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "Linux"
        };

        let platform_rule = if cfg!(target_os = "windows") {
            "- Host Platform is Windows. Executable binaries are PE (.exe), PowerShell and CMD are native."
        } else {
            "- Host Platform is Linux. Windows PE binaries (.exe) CANNOT run natively here. Always implement Linux/Cross-Platform solutions (Bash, Python, Flatpak, Rust ELF)."
        };

        if tools.is_empty() {
            return format!(
r#"You are Omni Assistant, an intelligent and helpful assistant running on {}.
- Answer the user inquiry directly, conversationally, and concisely.
- Do NOT output executable code blocks unless specifically requested by the user."#,
                os_name
            );
        }

        let mut tools_desc = Vec::new();
        let mut examples = Vec::new();

        if tools.contains(&ToolCapability::MemoryVfs) {
            tools_desc.push("  * `vfs.write(filepath, content)`: writes and stages file content into RAM VFS.");
            tools_desc.push("  * `vfs.read(filepath)`: reads text content from RAM VFS.");
            tools_desc.push("  * `print(message)`: outputs execution logs and status.");
        }

        if tools.contains(&ToolCapability::WebBrowser) {
            tools_desc.push("  * `browser.search(query)`: searches the web for latest news, headlines, or articles and returns structured text summaries. NEVER parse HTML or regex; write result directly to vfs.");
            tools_desc.push("  * `browser.open(url)`: fetches a web page and projects interactive elements into a clean ASCII grid.");
            tools_desc.push("  * `web.fetch(url)`: performs raw HTTP GET for specific URLs.");

            examples.push(r#"# Example: Fetching news and saving to file
news = browser.search("latest news")
vfs.write("today_news.txt", news)
print("News fetched and saved to RAM VFS.")"#);
        }

        if tools.contains(&ToolCapability::TerminalBridge) {
            tools_desc.push("  * `terminal.exec(command)`: runs a shell command on the host (e.g. package management, compiler, background launcher).");
            tools_desc.push("  * `terminal.logs(n)`: retrieves the last n lines of execution logs.");

            examples.push(r#"# Example: Running commands or launching software
terminal.exec("echo 'Launching task...'")
print("Command dispatched.")"#);
        }

        let tools_str = tools_desc.join("\n");
        let examples_str = if examples.is_empty() {
            "# Perform file operations\nvfs.write(\"output.txt\", \"data\")".to_string()
        } else {
            examples.join("\n\n")
        };

        format!(
r#"You are Omni Agent, an elite autonomous AI systems engineer running on {}.

[WORKSPACE & PLATFORM]:
- Platform: {}
- Embedded Executive Engine: You have direct access to an isolated execution sandbox (Bubblewrap bwrap / RAM tmpfs) with Python 3 and system shell.
- Execution Rule: Whenever requested to write code, build projects, or execute commands, ALWAYS output production-ready code inside standard markdown code blocks (e.g. ```python ... ```, ```bash ... ```) or tool actions.

[ACTIVE TOOLS SPECIFIC TO THIS GOAL]:
{}

[EXECUTION EXAMPLES]:
```python
{}
```

Execute with absolute precision. Output clean code blocks to accomplish the task."#,
            os_name,
            platform_rule,
            tools_str,
            examples_str
        )
    }

    pub fn build_system_2_grounded_prompt(
        tools: &[ToolCapability],
        something_i_know: &str,
        causal_feedback: Option<&str>,
    ) -> String {
        let base = Self::build_tailored_prompt(tools);
        let mut sections = Vec::new();
        sections.push(base);

        if !something_i_know.is_empty() {
            sections.push(format!(
r#"[SOMETHING I KNOW (SYSTEM 2 BLUEPRINT)]:
{}
Follow this blueprint strictly."#,
                something_i_know.trim()
            ));
        }

        if let Some(feedback) = causal_feedback {
            sections.push(format!(
r#"[CAUSAL FEEDBACK - DEAD END PRUNED VIA KV ROLLBACK]:
{}
CRITICAL INSTRUCTION: Your previous attempt trapped and its tokens were purged from the KV cache. Obey the causal lesson above and execute the fallback plan without repeating the trapped pattern."#,
                feedback.trim()
            ));
        }

        sections.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tailored_prompt_empty_tools() {
        let prompt = GroundedSystemProfile::build_tailored_prompt(&[]);
        assert!(prompt.contains("Omni Assistant"));
        assert!(!prompt.contains("vfs.write"));
    }

    #[test]
    fn test_tailored_prompt_browser_tools() {
        let prompt = GroundedSystemProfile::build_tailored_prompt(&[ToolCapability::WebBrowser, ToolCapability::MemoryVfs]);
        assert!(prompt.contains("browser.search"));
        assert!(prompt.contains("vfs.write"));
        assert!(!prompt.contains("terminal.exec"));
    }

    #[test]
    fn test_tailored_prompt_terminal_tools() {
        let prompt = GroundedSystemProfile::build_tailored_prompt(&[ToolCapability::TerminalBridge]);
        assert!(prompt.contains("terminal.exec"));
        assert!(!prompt.contains("browser.search"));
    }

    #[test]
    fn test_system_2_grounded_prompt_with_feedback() {
        let prompt = GroundedSystemProfile::build_system_2_grounded_prompt(
            &[ToolCapability::WebBrowser, ToolCapability::MemoryVfs],
            "* PLAN A: browser.search -> vfs.write",
            Some("Trap: attempt to index nil. Rule: use browser.search directly."),
        );
        assert!(prompt.contains("[SOMETHING I KNOW (SYSTEM 2 BLUEPRINT)]"));
        assert!(prompt.contains("* PLAN A: browser.search -> vfs.write"));
        assert!(prompt.contains("[CAUSAL FEEDBACK - DEAD END PRUNED VIA KV ROLLBACK]"));
        assert!(prompt.contains("Trap: attempt to index nil"));
    }
}
