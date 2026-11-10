# Contributing to Omni Engine

Thank you for your interest in contributing to **Omni Engine**, the sovereign, in-process autonomous software engineering runtime. We welcome community contributions, bug reports, and optimizations that align with our core architectural values: **100% offline determinism, cleanroom safety, and zero cloud lock-in**.

---

## 1. Contributor License Agreement (CLA) & Dual-Licensing Grant

Omni Engine is distributed under a **Dual-Licensing model**:
1. **Open Source:** GNU Affero General Public License v3.0 (AGPL-3.0).
2. **Commercial Enterprise:** Closed-source enterprise licenses for private corporate deployments.

### Legal Contribution Terms
To ensure Omni Engine can remain both vibrant open-source software and a sustainable enterprise project, all contributors must agree to the following terms prior to merging:

> **By submitting a pull request, patch, code contribution, or documentation update to the Omni Engine repository, you grant Ibrahim El Feqi (Omni AI) and his successors a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use, reproduce, adapt, modify, perform, display, sublicense, and distribute your contributions under both the GNU Affero General Public License v3.0 (AGPLv3) and any current or future commercial enterprise licenses without restriction or accounting.**

You represent that you are the sole author of your contributions, that you have the full legal right to grant these rights, and that your contribution does not violate any third-party intellectual property or employment agreements.

---

## 2. Engineering Standards & Architectural Invariants

All contributions must adhere strictly to the project's engineering discipline:

1. **Unit Test Coverage:** All new features or bug fixes must include unit tests. The test suite (`cargo test --lib`) must pass with **100% success (0 failures, 0 ignored)**.
2. **Compiler Quality:** The codebase must compile with **zero warnings and zero errors** across all targets (`cargo check --all-targets`).
3. **The Zero-Wipe Invariant ($W = \emptyset$):** Refactoring or modifying existing features must never introduce silent regressions or break existing public AST symbol contracts.
4. **Hermetic Sandboxing:** Features interacting with the host filesystem must strictly respect the `MemoryVfs` read/write set bounds and Bubblewrap (`bwrap`) cleanroom jail guarantees.
5. **Memory Safety & C-FFI Encapsulation:** Low-level bindings to `llama.cpp` must remain strictly encapsulated with verified lifetime bounds, preventing memory leaks and dangling pointers.

---

## 3. Development Workflow

### Prerequisites
- **Rust Toolchain:** Stable (1.80+), installed via [rustup.rs](https://rustup.rs).
- **Build Essentials:** `clang`, `cmake`, `libssl-dev`.
- **Linux Bubblewrap:** `bwrap` (for cleanroom testing).

### Local Build & Verification
```bash
# 1. Clone your fork
git clone https://github.com/<your-username>/omni_engine.git
cd omni_engine

# 2. Check compilation across all targets
cargo check --all-targets

# 3. Run the complete test suite
cargo test --lib

# 4. Build release binary
cargo build --release
```

---

## 4. Pull Request Guidelines

1. **Branch Naming:** Use descriptive branch prefixes:
   - `feat/` for new features (e.g., `feat/avx512-kv-quant`).
   - `fix/` for bug fixes (e.g., `fix/vfs-path-traversal`).
   - `docs/` for documentation updates.
2. **Commit Messages:** Follow the Conventional Commits specification (e.g., `feat(planner): optimize DAG makespan scheduler`).
3. **Keep PRs Focused:** Small, single-purpose PRs with clear rationale and test proofs are reviewed and merged significantly faster than monolithic changes.

---

## 5. Contact & Questions

For questions regarding contributions, architecture, or commercial licensing:
- **Founder & Maintainer:** Ibrahim El Feqi
- **Email:** [ielfeqi@gmail.com](mailto:ielfeqi@gmail.com)
- **GitHub Discussions / Issues:** [https://github.com/ielfeqi-rgb/omni_engine](https://github.com/ielfeqi-rgb/omni_engine)
