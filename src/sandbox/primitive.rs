use wasmi::{Engine, Linker, Module, Store, Val};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxDecisionGate {
    /// Sandbox trapped/failed: Purge attractor tokens via Causal KV Rollback
    RejectedNeedsCausalRollback { reason: String },
    /// Sandbox passed: Model can choose to execute an extra verification step in sandbox
    CanProceedToNextVerificationStep { return_value: Option<i64> },
    /// Sandbox passed: Model is ready and requests explicit user authorization before running on host
    RequiresUserHostAuthorization { return_value: Option<i64> },
}

#[derive(Debug, Clone)]
pub struct WasmExecutionOutcome {
    pub success: bool,
    pub return_value: Option<i64>,
    pub memory_dump: Vec<u8>,
    pub duration_us: u128,
    pub error_message: Option<String>,
    pub requires_user_authorization: bool,
}

impl WasmExecutionOutcome {
    /// Evaluate the dual-branch decision gate as required:
    /// If failed: immediate rejection & rollback.
    /// If success: model chooses either additional sandbox verification or requesting user authorization.
    pub fn evaluate_gate(&self, model_wants_further_check: bool) -> SandboxDecisionGate {
        if !self.success {
            return SandboxDecisionGate::RejectedNeedsCausalRollback {
                reason: self.error_message.clone().unwrap_or_else(|| "Unknown trap".to_string()),
            };
        }

        if model_wants_further_check {
            SandboxDecisionGate::CanProceedToNextVerificationStep {
                return_value: self.return_value,
            }
        } else {
            SandboxDecisionGate::RequiresUserHostAuthorization {
                return_value: self.return_value,
            }
        }
    }
}

/// Radical attack-surface reduction Sandbox.
/// Pure WASM bare-metal Stack Machine.
/// No sockets, no OS calls, no file system, purely linear RAM operations.
pub struct WasmPrimitiveSandbox {
    engine: Engine,
}

impl WasmPrimitiveSandbox {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    /// Execute a pure WASM bytecode binary with ZERO host-provided capabilities.
    /// Even if code tries to do syscalls, wasmi will reject it because the Linker is empty!
    pub fn execute_pure_wasm(
        &self,
        wasm_binary: &[u8],
        entry_fn: &str,
        args: &[i64],
    ) -> WasmExecutionOutcome {
        let start = Instant::now();

        // 1. Compile Module
        let module = match Module::new(&self.engine, wasm_binary) {
            Ok(m) => m,
            Err(e) => {
                return WasmExecutionOutcome {
                    success: false,
                    return_value: None,
                    memory_dump: Vec::new(),
                    duration_us: start.elapsed().as_micros(),
                    error_message: Some(format!("Module compilation error: {}", e)),
                    requires_user_authorization: false,
                };
            }
        };

        // 2. Initialize Store with empty state (Zero environment capabilities)
        let mut store = Store::new(&self.engine, ());

        // 3. Linker has NO external functions imported (No network, no files, zero OS capability)
        let linker = Linker::new(&self.engine);

        // 4. Instantiate module in pure isolated linear memory
        let instance = match linker.instantiate(&mut store, &module) {
            Ok(inst) => match inst.start(&mut store) {
                Ok(running) => running,
                Err(e) => {
                    return WasmExecutionOutcome {
                        success: false,
                        return_value: None,
                        memory_dump: Vec::new(),
                        duration_us: start.elapsed().as_micros(),
                        error_message: Some(format!("Module start trap: {}", e)),
                        requires_user_authorization: false,
                    };
                }
            },
            Err(e) => {
                return WasmExecutionOutcome {
                    success: false,
                    return_value: None,
                    memory_dump: Vec::new(),
                    duration_us: start.elapsed().as_micros(),
                    error_message: Some(format!("Instantiation rejected: {}", e)),
                    requires_user_authorization: false,
                };
            }
        };

        // 5. Look for target computation function
        let typed_func = match instance.get_func(&store, entry_fn) {
            Some(f) => f,
            None => {
                return WasmExecutionOutcome {
                    success: false,
                    return_value: None,
                    memory_dump: Vec::new(),
                    duration_us: start.elapsed().as_micros(),
                    error_message: Some(format!("Function '{}' not found in pure WASM module", entry_fn)),
                    requires_user_authorization: false,
                };
            }
        };

        // Convert input args
        let wasm_args: Vec<Val> = args.iter().map(|&a| Val::I64(a)).collect();
        let mut wasm_results = [Val::I64(0)];

        match typed_func.call(&mut store, &wasm_args, &mut wasm_results) {
            Ok(_) => {
                let ret = wasm_results[0].i64().unwrap_or(0);
                WasmExecutionOutcome {
                    success: true,
                    return_value: Some(ret),
                    memory_dump: Vec::new(),
                    duration_us: start.elapsed().as_micros(),
                    error_message: None,
                    // If it succeeded in isolated mathematical sandbox, it triggers the user authorization gate!
                    requires_user_authorization: true,
                }
            }
            Err(trap) => WasmExecutionOutcome {
                success: false,
                return_value: None,
                memory_dump: Vec::new(),
                duration_us: start.elapsed().as_micros(),
                error_message: Some(format!("Execution Trap (Isolated Protection): {}", trap)),
                requires_user_authorization: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pure_wasm_execution_without_os() {
        let sandbox = WasmPrimitiveSandbox::new();

        // Minimal valid WASM bytecode calculating: a + b (in 64-bit integers)
        // Handcrafted pure binary: zero OS imports, zero libc, zero network
        let wasm_add_bytes: [u8; 41] = [
            0x00, 0x61, 0x73, 0x6d, // \0asm (magic header)
            0x01, 0x00, 0x00, 0x00, // version 1
            // Type section (1): func(i64, i64) -> i64
            0x01, 0x07, 0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e,
            // Function section (3): decl func index 0
            0x03, 0x02, 0x01, 0x00,
            // Export section (7): export "add"
            0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
            // Code section (10): body of "add"
            0x0a, 0x09, 0x01, 0x07, 0x00,
            0x20, 0x00, // local.get 0
            0x20, 0x01, // local.get 1
            0x7c,       // i64.add
            0x0b        // end
        ];

        let res = sandbox.execute_pure_wasm(&wasm_add_bytes, "add", &[25, 17]);
        assert!(res.success, "Pure mathematical execution must succeed");
        assert_eq!(res.return_value, Some(42), "25 + 17 must equal 42");
        assert!(res.requires_user_authorization, "Successful execution must trigger user gate");
    }

    #[test]
    fn test_wasm_trapping_division_by_zero() {
        let sandbox = WasmPrimitiveSandbox::new();

        // Minimal valid WASM calculating: a / b (div_s)
        let wasm_div_bytes: [u8; 41] = [
            0x00, 0x61, 0x73, 0x6d,
            0x01, 0x00, 0x00, 0x00,
            0x01, 0x07, 0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e,
            0x03, 0x02, 0x01, 0x00,
            0x07, 0x07, 0x01, 0x03, 0x64, 0x69, 0x76, 0x00, 0x00,
            0x0a, 0x09, 0x01, 0x07, 0x00,
            0x20, 0x00, // local.get 0
            0x20, 0x01, // local.get 1
            0x7f,       // i64.div_s
            0x0b        // end
        ];

        // Trigger division by zero: 100 / 0
        let res = sandbox.execute_pure_wasm(&wasm_div_bytes, "div", &[100, 0]);
        assert!(!res.success, "Division by zero must be trapped safely");
        assert!(res.error_message.as_ref().unwrap().contains("integer divide by zero"));
        assert!(!res.requires_user_authorization, "Failed execution must not trigger user gate");

        // Gate evaluation on failure:
        let gate_fail = res.evaluate_gate(false);
        match gate_fail {
            SandboxDecisionGate::RejectedNeedsCausalRollback { reason } => {
                assert!(reason.contains("integer divide by zero"));
            }
            _ => panic!("Expected RejectedNeedsCausalRollback on trap"),
        }
    }

    #[test]
    fn test_sandbox_dual_branch_decision_gate() {
        let sandbox = WasmPrimitiveSandbox::new();
        let wasm_add_bytes: [u8; 41] = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x07, 0x01, 0x60, 0x02, 0x7e, 0x7e, 0x01, 0x7e,
            0x03, 0x02, 0x01, 0x00,
            0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
            0x0a, 0x09, 0x01, 0x07, 0x00,
            0x20, 0x00, 0x20, 0x01, 0x7c, 0x0b
        ];

        let res = sandbox.execute_pure_wasm(&wasm_add_bytes, "add", &[10, 20]);
        assert!(res.success);

        // Branch 1: Model wants another verification step
        let gate_step2 = res.evaluate_gate(true);
        assert_eq!(
            gate_step2,
            SandboxDecisionGate::CanProceedToNextVerificationStep { return_value: Some(30) }
        );

        // Branch 2: Model is confident and requests user authorization for host
        let gate_host = res.evaluate_gate(false);
        assert_eq!(
            gate_host,
            SandboxDecisionGate::RequiresUserHostAuthorization { return_value: Some(30) }
        );
    }
}
