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

//! Codebase Semantic Chunker.
//!
//! Decomposes raw source code files (Rust, C/C++, Python, Markdown, Shell)
//! into structurally meaningful semantic symbols (functions, structs, impl blocks, headings)
//! preserving file paths, line ranges, and symbols.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub file_path: String,
    pub symbol_name: String,
    pub chunk_text: String,
    pub line_start: usize,
    pub line_end: usize,
}

pub struct CodeChunker;

impl CodeChunker {
    /// Reads and chunks a file based on its extension and semantic markers
    pub fn chunk_file(file_path: &Path) -> Result<Vec<CodeChunk>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        let path_str = file_path.to_string_lossy().to_string();
        let ext = file_path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let chunks = match ext {
            "rs" => Self::chunk_rust(&content, &path_str),
            "py" => Self::chunk_python(&content, &path_str),
            "c" | "cpp" | "h" | "hpp" => Self::chunk_c(&content, &path_str),
            "md" => Self::chunk_markdown(&content, &path_str),
            _ => Self::chunk_generic(&content, &path_str, 40, 5),
        };

        Ok(chunks)
    }

    /// Chunks Rust code by searching for top-level constructs (fn, struct, enum, impl, trait)
    fn chunk_rust(content: &str, path_str: &str) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return chunks;
        }

        let mut current_start = 0;
        let mut current_symbol = "header".to_string();
        let mut buffer = Vec::new();

        for (idx, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // Detect new symbol boundary
            let is_boundary = trimmed.starts_with("pub fn ")
                || trimmed.starts_with("fn ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("trait ");

            if is_boundary && !buffer.is_empty() && (idx - current_start) >= 8 {
                chunks.push(CodeChunk {
                    file_path: path_str.to_string(),
                    symbol_name: current_symbol.clone(),
                    chunk_text: buffer.join("\n"),
                    line_start: current_start + 1,
                    line_end: idx,
                });
                buffer.clear();
                current_start = idx;
                current_symbol = extract_symbol_name(trimmed);
            } else if buffer.is_empty() && is_boundary {
                current_symbol = extract_symbol_name(trimmed);
            }

            buffer.push(line);

            // Cap chunk size at 60 lines
            if buffer.len() >= 60 {
                chunks.push(CodeChunk {
                    file_path: path_str.to_string(),
                    symbol_name: current_symbol.clone(),
                    chunk_text: buffer.join("\n"),
                    line_start: current_start + 1,
                    line_end: idx + 1,
                });
                buffer.clear();
                current_start = idx + 1;
                current_symbol = format!("block_L{}", current_start + 1);
            }
        }

        if !buffer.is_empty() {
            chunks.push(CodeChunk {
                file_path: path_str.to_string(),
                symbol_name: current_symbol,
                chunk_text: buffer.join("\n"),
                line_start: current_start + 1,
                line_end: lines.len(),
            });
        }

        chunks
    }

    /// Chunks Python code by `def ` and `class `
    fn chunk_python(content: &str, path_str: &str) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return chunks;
        }

        let mut current_start = 0;
        let mut current_symbol = "module".to_string();
        let mut buffer = Vec::new();

        for (idx, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let is_boundary = (trimmed.starts_with("def ") || trimmed.starts_with("class "))
                && !line.starts_with(" ");

            if is_boundary && !buffer.is_empty() && (idx - current_start) >= 6 {
                chunks.push(CodeChunk {
                    file_path: path_str.to_string(),
                    symbol_name: current_symbol.clone(),
                    chunk_text: buffer.join("\n"),
                    line_start: current_start + 1,
                    line_end: idx,
                });
                buffer.clear();
                current_start = idx;
                current_symbol = extract_symbol_name(trimmed);
            }

            buffer.push(line);

            if buffer.len() >= 60 {
                chunks.push(CodeChunk {
                    file_path: path_str.to_string(),
                    symbol_name: current_symbol.clone(),
                    chunk_text: buffer.join("\n"),
                    line_start: current_start + 1,
                    line_end: idx + 1,
                });
                buffer.clear();
                current_start = idx + 1;
                current_symbol = format!("block_L{}", current_start + 1);
            }
        }

        if !buffer.is_empty() {
            chunks.push(CodeChunk {
                file_path: path_str.to_string(),
                symbol_name: current_symbol,
                chunk_text: buffer.join("\n"),
                line_start: current_start + 1,
                line_end: lines.len(),
            });
        }

        chunks
    }

    /// Chunks C/C++ files
    fn chunk_c(content: &str, path_str: &str) -> Vec<CodeChunk> {
        Self::chunk_generic(content, path_str, 50, 5)
    }

    /// Chunks Markdown by headings (# Heading)
    fn chunk_markdown(content: &str, path_str: &str) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return chunks;
        }

        let mut current_start = 0;
        let mut current_heading = "intro".to_string();
        let mut buffer = Vec::new();

        for (idx, &line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let is_heading = trimmed.starts_with("# ")
                || trimmed.starts_with("## ")
                || trimmed.starts_with("### ");

            if is_heading && !buffer.is_empty() {
                chunks.push(CodeChunk {
                    file_path: path_str.to_string(),
                    symbol_name: current_heading.clone(),
                    chunk_text: buffer.join("\n"),
                    line_start: current_start + 1,
                    line_end: idx,
                });
                buffer.clear();
                current_start = idx;
                current_heading = trimmed.trim_start_matches('#').trim().to_string();
            }

            buffer.push(line);
        }

        if !buffer.is_empty() {
            chunks.push(CodeChunk {
                file_path: path_str.to_string(),
                symbol_name: current_heading,
                chunk_text: buffer.join("\n"),
                line_start: current_start + 1,
                line_end: lines.len(),
            });
        }

        chunks
    }

    /// Generic sliding window chunker
    fn chunk_generic(content: &str, path_str: &str, window_size: usize, overlap: usize) -> Vec<CodeChunk> {
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        if total == 0 {
            return chunks;
        }

        let mut i = 0;
        while i < total {
            let end = (i + window_size).min(total);
            let slice = &lines[i..end];
            chunks.push(CodeChunk {
                file_path: path_str.to_string(),
                symbol_name: format!("L{}-L{}", i + 1, end),
                chunk_text: slice.join("\n"),
                line_start: i + 1,
                line_end: end,
            });

            if end == total {
                break;
            }
            i += window_size.saturating_sub(overlap).max(1);
        }

        chunks
    }
}

fn extract_symbol_name(line: &str) -> String {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        let name = parts[1].split('(').next().unwrap_or(parts[1]);
        name.trim_end_matches('{').trim().to_string()
    } else {
        "symbol".to_string()
    }
}
