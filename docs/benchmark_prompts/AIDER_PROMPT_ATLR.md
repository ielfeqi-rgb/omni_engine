# Human Briefing for Aider: Atomic Transaction Ledger in Rust (ATLR)

You are tasked with implementing the **Atomic Transaction Ledger in Rust (ATLR)**.

## Project Constraints
- **Language**: Rust (Edition 2021).
- **Dependencies**: STRICTLY ZERO external crates (`std` only). Do NOT add `parking_lot`, `tokio`, `crossbeam`, `rayon`, or anything to `Cargo.toml`.
- **Safety**: STRICTLY ZERO `unsafe` blocks. `#![forbid(unsafe_code)]` and `#![deny(unused_must_use)]` must be placed at the top of `src/lib.rs`.

---

## Technical Specifications

### 1. Account Model (`src/models.rs`)
- Each account has:
  - `id: u64` (unique ID).
  - `balance: i64`.
- Define error enum `TransferError`:
  - `AccountNotFound(u64)`
  - `InsufficientFunds { available: i64, required: i64 }`
  - `SameAccountTransfer(u64)`
  - `InvalidAmount(i64)`
  - `LockPoisoned`

### 2. Transaction Engine (`src/ledger.rs`)
Implement `FinancialLedger` with:
- Method `transfer(&self, from_id: u64, to_id: u64, amount: i64) -> Result<(), TransferError>`
- **Deadlock Prevention (CRITICAL)**:
  - If multiple threads transfer between the same accounts concurrently in opposing directions ($A \to B$ and $B \to A$), deadlocks must be mathematically impossible.
  - You MUST acquire locks in canonical order based on account ID (e.g. always acquire `min(from_id, to_id)` before `max(from_id, to_id)`).
- **The Fee Rule**:
  - A fixed fee of `1` unit is charged to the sender for every transfer attempt.
- **The Rollback Rule (CRITICAL)**:
  - The transfer and fee are atomic.
  - Deduct the `1` unit fee from sender first.
  - If the sender's remaining balance is strictly less than `amount`, the system MUST rollback the fee (restore the `1` unit to sender) and return `Err(TransferError::InsufficientFunds)`.
- **System Liquidity Conservation**:
  - All fees deducted must be credited to a designated `fee_account_id` (e.g., Account 0 / Treasury) so that the total system liquidity is conserved.
  - Method `total_system_liquidity(&self) -> Result<i64, TransferError>` must sum all account balances.

### 3. Stress Simulator (`tests/atlr_stress.rs`)
- Create an integration test that:
  1. Initializes 3 user accounts (Accounts 1, 2, 3) with `10,000` units each (total = `30,000`).
  2. Initializes Account 0 with `0` as the Fee Collector.
  3. Spawns 50 concurrent threads executing rapid reciprocal transfers ($1 \to 2$, $2 \to 3$, $3 \to 1$, $2 \to 1$, etc.) in a Thundering Herd burst.
  4. Asserts that all 50 threads complete without deadlocks.
  5. Asserts that no account ever drops below 0 balance.
  6. Asserts that the final total system liquidity is EXACTLY `30,000` units (Zero Money Lost, Zero Money Duplicated).
  7. Asserts the rollback invariant: an attempted transfer exceeding balance fails cleanly and leaks zero fees.

Please generate `src/models.rs`, `src/ledger.rs`, `src/lib.rs`, and `tests/atlr_stress.rs`. Ensure `cargo test` passes cleanly with 0 warnings.
