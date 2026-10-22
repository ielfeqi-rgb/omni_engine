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

use tracing::info;

pub struct AstSlice {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
}

pub struct TopologicalDiagnostic;

impl TopologicalDiagnostic {
    /// Extracts the precise block of code (function/class or window) containing the error
    /// in a 100% language-agnostic manner in pure Rust without external interpreter dependencies.
    pub fn extract_broken_node(workspace_dir: &str, file_path: &str, target_line: usize) -> Result<AstSlice, String> {
        let full_target_path = format!("{}/{}", workspace_dir, file_path);
        let content = std::fs::read_to_string(&full_target_path)
            .or_else(|_| std::fs::read_to_string(file_path))
            .map_err(|e| format!("Failed to read target file {}: {}", file_path, e))?;

        Self::extract_from_source(file_path, &content, target_line)
    }

    /// Pure in-memory AST / block extractor that works with VFS or disk content across any language
    pub fn extract_from_source(file_path: &str, content: &str, target_line: usize) -> Result<AstSlice, String> {
        let lines: Vec<&str> = content.split('\n').collect();
        if lines.is_empty() {
            return Err("Empty file content".to_string());
        }

        let total_lines = lines.len();
        let target_idx = target_line.saturating_sub(1).min(total_lines.saturating_sub(1));

        // Language-Agnostic Block Detection:
        // Scan backwards to find the start of the enclosing scope or definition
        let mut start_idx = target_idx.saturating_sub(6);
        for i in (0..=target_idx).rev() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("def ")
                || trimmed.starts_with("async def ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("function ")
                || trimmed.starts_with("export ")
                || trimmed.starts_with("public ")
                || trimmed.starts_with("private ")
                || trimmed.starts_with("protected ")
                || trimmed.starts_with("void ")
                || trimmed.starts_with("int ")
                || trimmed.starts_with("double ")
                || trimmed.starts_with("float ")
                || trimmed.starts_with("char ")
                || trimmed.starts_with("func ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl ")
                || ((trimmed.starts_with("const ") || trimmed.starts_with("let ") || trimmed.starts_with("var ")) && (trimmed.contains("=>") || trimmed.contains("function")))
                || (trimmed.ends_with('{') && !trimmed.starts_with("if") && !trimmed.starts_with("for") && !trimmed.starts_with("while") && !trimmed.starts_with("else"))
            {
                start_idx = i;
                break;
            }
            if target_idx - i > 12 {
                break;
            }
        }

        // Scan forwards to find the end of the enclosing scope or a reasonable window
        let mut end_idx = (target_idx + 8).min(total_lines);
        for i in target_idx..total_lines {
            let trimmed = lines[i].trim();
            if (trimmed == "}" || trimmed == "end") && i > target_idx {
                end_idx = i + 1;
                break;
            }
            if i - target_idx > 15 {
                break;
            }
        }

        let start_line = start_idx + 1;
        let end_line = end_idx;

        let mut slice_content = String::new();
        for i in start_idx..end_idx.min(total_lines) {
            slice_content.push_str(lines[i]);
            slice_content.push('\n');
        }

        info!("Language-agnostic surgical slice extracted: {} lines {}-{}", file_path, start_line, end_line);

        Ok(AstSlice {
            file_path: file_path.to_string(),
            start_line,
            end_line,
            content: slice_content,
        })
    }
}
