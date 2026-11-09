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

pub mod executive_hands;
pub mod pre_pass_triage;
pub mod supervisor;
pub mod system_profile;
pub mod memory_gate;
pub mod action_parser;
pub mod swarm_coordinator;
pub mod dossier;

pub use executive_hands::{ExecutiveHands, ExecutiveAction};
pub use pre_pass_triage::{ExecutionIntent, PrePassTriage, DualSystemPlan, SpeculativeTarget};
pub use supervisor::{InternalSupervisorProbe, AncestralTestament};
pub use system_profile::{GroundedSystemProfile, ToolCapability};
pub use memory_gate::{MemoryGate, MemoryGateError};
pub use action_parser::{
    SwarmAction, DiagnosticCode, ThinkerDecision, DependencyChoice, HostPackageManager,
    strip_thought, strip_command_prefix, parse_action, parse_thinker_decision, extract_missing_dependency,
};
pub use swarm_coordinator::{SwarmCoordinator, SwarmConfig, SwarmResult, SubGoal, WorkerFinding};
pub use dossier::{CustomerDossier, PlannerDistiller, DistillationDelta, PurchaseRecord};
pub use fixer::{CodeFixer, Diagnostic};
pub use observer::{SovereignObserver, ObserverDirective};

pub mod observer;
pub mod fixer;
pub mod analyzer;
pub mod oracle;
pub mod refinement_loop;
