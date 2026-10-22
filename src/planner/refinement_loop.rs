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

use crate::native_llama::{NativeLlamaModel, NativeLlamaContext};
use crate::planner::analyzer::{StaticAnalyzer, AnalyzerResult, Chunk};
use crate::planner::surgeon::{MicroSurgeon, Diagnostic};
use crate::planner::oracle::TerminalOracle;
use std::sync::{Arc, Mutex};
use tracing::{warn, error};

pub const MAX_SYNTAX_RETRIES: usize = 3;
pub const MAX_BEHAVIORAL_RETRIES: usize = 2;

#[derive(Debug, Clone)]
pub enum RefinementState {
    StaticAnalysis,
    Surgery {
        diagnostic: Diagnostic,
        syntax_retry_count: usize,
    },
    TerminalExecution {
        command: String,
        syntax_retry_count: usize,
        behavioral_retry_count: usize,
    },
    BehavioralVerification {
        command: String,
        terminal_output: String,
        syntax_retry_count: usize,
        behavioral_retry_count: usize,
    },
    MacroFallback {
        reason: String,
    },
    SuccessVerified,
    FatalError(String),
}

pub struct RefinementPipeline {
    analyzer: StaticAnalyzer,
    surgeon: MicroSurgeon,
    oracle: TerminalOracle,
    pub current_vfs_code: String,
    pub filename: String,
    original_goal: String,
    source_seq_id: i32,
    ephemeral_seq_id: i32,
}

impl RefinementPipeline {
    pub fn new(
        model: Arc<NativeLlamaModel>,
        ctx: Arc<Mutex<NativeLlamaContext>>,
        initial_code: String,
        filename: String,
        original_goal: String,
        source_seq_id: i32,
    ) -> Self {
        Self {
            analyzer: StaticAnalyzer::new(model.clone(), ctx.clone()),
            surgeon: MicroSurgeon::new(model.clone(), ctx.clone()),
            oracle: TerminalOracle::new(model.clone(), ctx.clone()),
            current_vfs_code: initial_code,
            filename,
            original_goal,
            source_seq_id,
            ephemeral_seq_id: source_seq_id + 10, // Safe offset for zero-shot clones
        }
    }

    /// Extends the VFS mocking by returning a chunk of code.
    fn get_code_chunk(&self) -> Chunk {
        Chunk {
            start_line: 0,
            end_line: self.current_vfs_code.lines().count(),
            content: self.current_vfs_code.clone(),
        }
    }

    /// Utility to mock patching a line in the VFS
    fn patch_line(&mut self, line_number: usize, new_line: &str) {
        let lines: Vec<&str> = self.current_vfs_code.lines().collect();
        let mut new_code = String::new();
        for (i, line) in lines.iter().enumerate() {
            if i + 1 == line_number {
                new_code.push_str(new_line);
            } else {
                new_code.push_str(line);
            }
            new_code.push('\n');
        }
        self.current_vfs_code = new_code;
    }

    /// The Main Loop FSM
    pub fn run(&mut self) -> RefinementState {
        let mut state = RefinementState::StaticAnalysis;
        let mut syntax_retries = 0;
        let mut behavioral_retries = 0;

        loop {
            println!("  [FSM] Transition -> {:?}", state);
            match state {
                RefinementState::StaticAnalysis => {
                    let chunk = self.get_code_chunk();
                    match self.analyzer.evaluate_chunk(self.ephemeral_seq_id, &self.filename, &chunk) {
                        Ok(AnalyzerResult::IssueFound { line_number, error_message, lines_context: _ }) => {
                            state = RefinementState::Surgery {
                                diagnostic: Diagnostic { line_number, error_message },
                                syntax_retry_count: syntax_retries,
                            };
                        }
                        Ok(AnalyzerResult::Pass { execution_command: Some(cmd) }) => {
                            state = RefinementState::TerminalExecution {
                                command: cmd,
                                syntax_retry_count: syntax_retries,
                                behavioral_retry_count: behavioral_retries,
                            };
                        }
                        Ok(AnalyzerResult::Pass { execution_command: None }) => {
                            state = RefinementState::SuccessVerified;
                        }
                        Err(e) => {
                            state = RefinementState::FatalError(e);
                        }
                    }
                }

                RefinementState::Surgery { diagnostic, syntax_retry_count } => {
                    if syntax_retry_count >= MAX_SYNTAX_RETRIES {
                        warn!("Circuit Breaker: Max syntax retries reached.");
                        state = RefinementState::MacroFallback {
                            reason: "Exceeded MAX_SYNTAX_RETRIES during micro-surgery".to_string(),
                        };
                        continue;
                    }

                    syntax_retries += 1;

                    // Extract context for the surgeon (simple 3-line extraction logic mock)
                    let context_lines = &self.current_vfs_code;

                    match self.surgeon.perform_surgery(self.source_seq_id, self.ephemeral_seq_id, context_lines, &diagnostic) {
                        Ok(fixed_line) => {
                            self.patch_line(diagnostic.line_number, &fixed_line);
                            state = RefinementState::StaticAnalysis; // Go back to check
                        }
                        Err(e) => {
                            error!("Surgeon failed: {}", e);
                            state = RefinementState::FatalError(e);
                        }
                    }
                }

                RefinementState::TerminalExecution { command, syntax_retry_count, behavioral_retry_count } => {
                    let res = self.oracle.execute_command(&command, &self.filename, &self.current_vfs_code);
                    if res.exit_code == 0 {
                        state = RefinementState::BehavioralVerification {
                            command,
                            terminal_output: res.output,
                            syntax_retry_count,
                            behavioral_retry_count,
                        };
                    } else {
                        // Execution failed, diagnose using AI
                        match self.oracle.diagnose_crash(self.source_seq_id, self.ephemeral_seq_id, &res.output) {
                            Ok(Some(diag)) => {
                                state = RefinementState::Surgery {
                                    diagnostic: diag,
                                    syntax_retry_count: syntax_retries,
                                };
                            }
                            Ok(None) => {
                                state = RefinementState::MacroFallback {
                                    reason: "Terminal crashed and AI could not diagnose the specific line.".to_string(),
                                };
                            }
                            Err(e) => {
                                state = RefinementState::FatalError(e);
                            }
                        }
                    }
                }

                RefinementState::BehavioralVerification { command: _, terminal_output, syntax_retry_count: _, behavioral_retry_count } => {
                    if behavioral_retry_count >= MAX_BEHAVIORAL_RETRIES {
                        warn!("Circuit Breaker: Max behavioral retries reached.");
                        state = RefinementState::MacroFallback {
                            reason: "Exceeded MAX_BEHAVIORAL_RETRIES".to_string(),
                        };
                        continue;
                    }
                    
                    behavioral_retries += 1;

                    match self.oracle.verify_behavior(self.source_seq_id, self.ephemeral_seq_id, &self.original_goal, &terminal_output) {
                        Ok(true) => {
                            state = RefinementState::SuccessVerified;
                        }
                        Ok(false) => {
                            // Valid execution, but wrong behavior. 
                            state = RefinementState::MacroFallback {
                                reason: "Program compiled but failed behavioral verification.".to_string(),
                            };
                        }
                        Err(e) => {
                            state = RefinementState::FatalError(e);
                        }
                    }
                }

                RefinementState::SuccessVerified | RefinementState::MacroFallback { .. } | RefinementState::FatalError(_) => {
                    // Terminal states
                    break;
                }
            }
        }
        state
    }
}
