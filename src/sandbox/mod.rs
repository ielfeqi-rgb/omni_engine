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

pub mod diagnostics;
pub mod vfs;
pub mod browser_lens;
pub mod terminal_bridge;
pub mod web_lens;
pub mod isolated_jail;
pub mod cleanroom;
pub mod ui_verifier;

pub use vfs::{MemoryVfs, FileChangeType, VfsBoundaryError};
pub use browser_lens::{BrowserTerminalLens, InteractiveElement, ActionTargetType};
pub use terminal_bridge::{TerminalSessionBridge, JobStatus};
pub use web_lens::{WebLens, SearchResult};
pub use isolated_jail::{IsolatedJail, JailExecutionResult};
pub use cleanroom::{UniversalCleanroom, WrittenAnalysisReport, CodeAnalysisReport, InteractiveExecutionReport, AcceptanceDeltaReport, TestSheet};
pub use ui_verifier::{UiContractVerifier, UiTestStep, UiInspectionReport};


