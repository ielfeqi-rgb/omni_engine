# tools/ -- Project Guidance & Audit Reports

This folder contains audit reports, cleanup notes, and the roadmap
created during the 2026-09-23 review session.

## Contents

### reports/
- **project_audit.md** -- Full honest audit: what each module does, what each benchmark actually tests, what is real vs naming
- **maintenance_report.md** -- 76 unwrap risks, 52 swallowed errors, hardcoded paths, zero-coverage modules, prioritized fix list
- **kv_cache_feasibility.md** -- Can the engine implement real KV-cache theories? 3 upgrade paths with effort estimates

### roadmap/
- **ROADMAP.md** -- The correct build sequence: Phase 0 (stabilize) through Phase 4 (real swarm). Each phase validates the next.

### deprecated/
- Reserved for files moved out of active use during cleanup

### Root
- **CLEANUP_NOTES.md** -- Specific items to remove or rename, with exact file/line references and why

## How to Use

1. Start with **ROADMAP.md** -- it tells you what to do in what order
2. Use **CLEANUP_NOTES.md** as a checklist while fixing code
3. Reference **maintenance_report.md** for the exact line numbers of each issue
4. Read **kv_cache_feasibility.md** only after Phase 1 (first real model test) is done

## Inline Source Guidance

Source files have been annotated with `// [GUIDANCE]` and `// [FIX]` comments
at critical locations. Search for them:

    grep -rn '\[GUIDANCE\]\|\[FIX\]' src/
