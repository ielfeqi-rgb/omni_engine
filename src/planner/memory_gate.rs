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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryGateError {
    OverCapacity { requested: usize, k_max: usize },
    InsufficientCapacity { requested: usize, remaining: usize, k_max: usize },
}

#[derive(Debug, Clone)]
pub struct MemoryGate {
    pub k_max: usize,
    pub active_tokens: usize,
    pub bytes_per_token: usize,
}

impl MemoryGate {
    pub fn new(k_max: usize) -> Self {
        Self::with_bytes_per_token(k_max, 28_672)
    }

    pub fn with_bytes_per_token(k_max: usize, bytes_per_token: usize) -> Self {
        Self {
            k_max,
            active_tokens: 0,
            bytes_per_token,
        }
    }

    #[inline]
    pub fn can_admit(&self, requested: usize) -> bool {
        if self.is_over_capacity(requested) {
            return false;
        }
        self.active_tokens.saturating_add(requested) <= self.k_max
    }

    #[inline]
    pub fn is_over_capacity(&self, requested: usize) -> bool {
        requested > self.k_max
    }

    pub fn allocate(&mut self, tokens: usize) -> Result<(), ()> {
        if self.can_admit(tokens) {
            self.active_tokens = self.active_tokens.saturating_add(tokens);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn try_allocate(&mut self, tokens: usize) -> Result<(), MemoryGateError> {
        if self.is_over_capacity(tokens) {
            return Err(MemoryGateError::OverCapacity {
                requested: tokens,
                k_max: self.k_max,
            });
        }
        if self.active_tokens.saturating_add(tokens) > self.k_max {
            return Err(MemoryGateError::InsufficientCapacity {
                requested: tokens,
                remaining: self.remaining(),
                k_max: self.k_max,
            });
        }
        self.active_tokens = self.active_tokens.saturating_add(tokens);
        Ok(())
    }

    pub fn release(&mut self, tokens: usize) {
        self.active_tokens = self.active_tokens.saturating_sub(tokens);
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.k_max.saturating_sub(self.active_tokens)
    }

    #[inline]
    pub fn active_bytes(&self) -> usize {
        self.active_tokens * self.bytes_per_token
    }

    #[inline]
    pub fn max_bytes(&self) -> usize {
        self.k_max * self.bytes_per_token
    }

    #[inline]
    pub fn utilization(&self) -> f64 {
        if self.k_max == 0 {
            0.0
        } else {
            (self.active_tokens as f64 / self.k_max as f64).clamp(0.0, 1.0)
        }
    }

    /// Analytical calculation of bytes per token for FP16 KV-Cache:
    /// 4 * L * H_kv * (D / H_q)
    #[inline]
    pub fn analytical_bytes_per_token(l: usize, h_kv: usize, d: usize, h_q: usize) -> usize {
        let d_head = if h_q > 0 { d / h_q } else { 0 };
        4 * l * h_kv * d_head
    }

    /// Global Token Capacity Ceiling:
    /// min(n_ctx_train, floor(m_budget / bytes_per_token))
    #[inline]
    pub fn calculate_k_max(n_ctx_train: usize, m_budget_bytes: usize, bytes_per_token: usize) -> usize {
        let k_mem = if bytes_per_token > 0 {
            m_budget_bytes / bytes_per_token
        } else {
            n_ctx_train
        };
        n_ctx_train.min(k_mem)
    }
}
