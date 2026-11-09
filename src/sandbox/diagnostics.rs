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

#[derive(Debug, Clone)]
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
                || trimmed.starts_with("pub class ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("pub interface ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("pub type ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("pub trait ")
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

        // Language-Agnostic Attribute & Decorator Absorption:
        // Absorb preceding macro attributes (#[...]), decorators (@...), and doc comments (///, /**, *)
        // so that derive attributes and annotations are fully included in the surgical focal block.
        while start_idx > 0 {
            let prev = lines[start_idx - 1].trim();
            if prev.starts_with("#[")
                || prev.starts_with('@')
                || prev.starts_with("///")
                || prev.starts_with("//!")
                || prev.starts_with("/**")
                || prev.starts_with('*')
            {
                start_idx -= 1;
            } else {
                break;
            }
        }

        // Scan forwards to find the end of the enclosing scope with brace balancing
        let mut brace_depth: i32 = 0;
        let mut end_idx = (target_idx + 25).min(total_lines);
        for (i, line) in lines.iter().enumerate().take(total_lines).skip(start_idx) {
            brace_depth += line.matches('{').count() as i32;
            brace_depth -= line.matches('}').count() as i32;
            let trimmed = line.trim();
            if i >= target_idx {
                if brace_depth <= 0 && (trimmed == "}" || trimmed.ends_with('}') || trimmed == "end") {
                    end_idx = i + 1;
                    break;
                }
                if i - target_idx > 80 {
                    end_idx = i + 1;
                    break;
                }
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

    /// Locates the 1-based line number of a named symbol (function, method, struct, enum, class) in source code
    pub fn find_symbol_line(content: &str, symbol_name: &str) -> Option<usize> {
        let clean_sym = symbol_name.trim();
        if clean_sym.is_empty() {
            return None;
        }
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if (trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("def ") || trimmed.starts_with("async def ")
                || trimmed.starts_with("struct ") || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("class ") || trimmed.starts_with("trait ")
                || trimmed.starts_with("impl "))
                && line.contains(clean_sym)
            {
                return Some(idx + 1);
            }
        }
        None
    }

    /// Extracts the AST slice for a specific named symbol
    pub fn extract_symbol_slice(file_path: &str, content: &str, symbol_name: &str) -> Result<AstSlice, String> {
        if let Some(line) = Self::find_symbol_line(content, symbol_name) {
            Self::extract_from_source(file_path, content, line)
        } else {
            Err(format!("Symbol '{}' not found in '{}'", symbol_name, file_path))
        }
    }

    /// Extracts the verified preceding block before a target line to serve as a Read-Only Attention Anchor.
    pub fn extract_preceding_anchor(file_path: &str, content: &str, target_start_line: usize) -> Option<AstSlice> {
        if target_start_line <= 1 {
            return None;
        }
        let lines: Vec<&str> = content.lines().collect();
        let target_idx = target_start_line.saturating_sub(1).min(lines.len());
        if target_idx == 0 {
            return None;
        }

        // Look backwards for a preceding symbol definition or enclosing struct/enum
        let mut anchor_line = None;
        for i in (0..target_idx).rev() {
            let trimmed = lines[i].trim();
            if trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("def ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
            {
                anchor_line = Some(i + 1);
                break;
            }
        }

        if let Some(line) = anchor_line {
            Self::extract_from_source(file_path, content, line).ok()
        } else {
            let start = target_idx.saturating_sub(15);
            let slice_content = lines[start..target_idx].join("\n");
            Some(AstSlice {
                file_path: file_path.to_string(),
                start_line: start + 1,
                end_line: target_idx,
                content: slice_content,
            })
        }
    }

    /// Extracts concise single-line essentials / skeletons of all defined symbols in the file.
    /// Never includes internal 20-line implementation bodies to avoid confusing small LLMs.
    pub fn extract_block_essentials(content: &str) -> Vec<String> {
        let mut essentials = Vec::new();
        for line in content.lines() {
            let tr = line.trim();
            if tr.starts_with("//") || tr.starts_with('#') || tr.starts_with('*') {
                continue;
            }
            if tr.starts_with("pub enum ") || tr.starts_with("enum ")
                || tr.starts_with("pub struct ") || tr.starts_with("struct ")
                || tr.starts_with("pub trait ") || tr.starts_with("trait ")
                || tr.starts_with("class ")
            {
                let sig = tr.split('{').next().unwrap_or(tr).split(':').next().unwrap_or(tr).trim();
                if !sig.is_empty() {
                    essentials.push(sig.to_string());
                }
            } else if tr.starts_with("pub fn ") || tr.starts_with("fn ")
                || tr.starts_with("pub async fn ") || tr.starts_with("async fn ")
                || tr.starts_with("def ") || tr.starts_with("async def ")
            {
                let sig = tr.split('{').next().unwrap_or(tr).split(':').next().unwrap_or(tr).trim();
                if !sig.is_empty() {
                    essentials.push(sig.to_string());
                }
            }
        }
        essentials
    }

    /// Extracts the clean identifier names of all symbols defined in the file
    pub fn extract_defined_symbol_names(content: &str) -> Vec<String> {
        let mut names = Vec::new();
        for line in content.lines() {
            let tr = line.trim();
            if tr.starts_with("//") || tr.starts_with('#') || tr.starts_with('*') {
                continue;
            }
            let is_symbol = tr.starts_with("pub enum ") || tr.starts_with("enum ")
                || tr.starts_with("pub struct ") || tr.starts_with("struct ")
                || tr.starts_with("pub trait ") || tr.starts_with("trait ")
                || tr.starts_with("class ")
                || tr.starts_with("pub fn ") || tr.starts_with("fn ")
                || tr.starts_with("pub async fn ") || tr.starts_with("async fn ")
                || tr.starts_with("def ") || tr.starts_with("async def ");

            if is_symbol {
                let parts: Vec<&str> = tr.split_whitespace().collect();
                let candidate = if parts.len() > 1 && (parts[0] == "pub" || parts[0] == "async") {
                    if parts.len() > 2 && parts[1] == "async" {
                        parts.get(3).copied()
                    } else {
                        parts.get(2).copied()
                    }
                } else {
                    parts.get(1).copied()
                };

                if let Some(raw_name) = candidate {
                    let clean_name = raw_name
                        .split(&['(', '<', '{', ':', ';', ' '][..])
                        .next()
                        .unwrap_or(raw_name)
                        .trim();
                    if !clean_name.is_empty() && !names.contains(&clean_name.to_string()) {
                        names.push(clean_name.to_string());
                    }
                }
            }
        }
        names
    }

    /// Excises duplicate sealed contract blocks from model output if the model inadvertently re-emitted
    /// a definition for a symbol that is already inherent and sealed in the target file.
    pub fn excise_duplicate_sealed_blocks(
        file_path: &str,
        output_content: &str,
        sealed_symbols: &[String],
    ) -> String {
        let mut cleaned = output_content.to_string();
        for sym in sealed_symbols {
            if sym.is_empty() || sym == "new" || sym == "default" {
                continue;
            }
            if let Some(sym_line) = Self::find_symbol_line(&cleaned, sym) {
                if let Ok(slice) = Self::extract_from_source(file_path, &cleaned, sym_line) {
                    let lines: Vec<&str> = cleaned.lines().collect();
                    if lines.len() > slice.end_line - slice.start_line + 2 {
                        let start_idx = slice.start_line.saturating_sub(1);
                        let end_idx = slice.end_line.min(lines.len());
                        let mut new_lines = lines[..start_idx].to_vec();
                        if end_idx < lines.len() {
                            new_lines.extend_from_slice(&lines[end_idx..]);
                        }
                        cleaned = new_lines.join("\n");
                    }
                }
            }
        }
        cleaned
    }
}
