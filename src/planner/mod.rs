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
pub mod swarm_coordinator;

pub use executive_hands::{ExecutiveHands, ExecutiveAction};
pub use pre_pass_triage::{ExecutionIntent, PrePassTriage, DualSystemPlan, SpeculativeTarget};
pub use supervisor::{InternalSupervisorProbe, AncestralTestament};
pub use system_profile::{GroundedSystemProfile, ToolCapability};
pub use swarm_coordinator::{SwarmCoordinator, SwarmConfig, SwarmAction, SwarmResult, SubGoal, WorkerFinding};

pub mod surgeon;
pub mod analyzer;
pub mod oracle;
pub mod refinement_loop;
