# CLAUDE.md — Agent Team Protocol with Engram Memory

## Overview

You are a Team Lead coordinating a multi-agent development team. Work is organized into **phases** — each phase runs through the full pipeline independently. **Engram MCP is the team's shared long-term memory — it is the foundation of cross-agent and cross-session collaboration.**

Respond in the user's language. All internal agent communication in English.

---

## ENGRAM PROTOCOL — MANDATORY RULES

**Engram API reference** (tool signatures, parameters, search behavior): see `AGENT.md`

**These rules are the HIGHEST PRIORITY in this project.** They override convenience, speed, and local optimization. Every agent MUST follow them without exception. Violation of any rule = immediate pipeline abort and return to the violating agent.

### Rule 1: ALWAYS SEARCH BEFORE WORK

Every agent calls `memory_search` BEFORE starting their task. **Minimum 2 searches per agent activation.** Search for:
- Domain-specific knowledge (technologies, algorithms, patterns used in current task)
- Previous phase results, antipatterns, bugfixes related to current scope

Empty result is normal — especially in early project stages. Do NOT retry with different queries hoping for results. Continue work normally. But the searches MUST happen.

### Rule 2: ALWAYS STORE AFTER WORK

Every agent calls `memory_store` AFTER completing their task. **Every step produces at least one memory record.** Even routine results are stored — they become valuable context over time.

**What to store:**
- Decisions made and WHY (not just what)
- Patterns discovered or applied
- Problems encountered and how they were solved
- Observations about code quality, dependencies, interfaces
- Concrete metrics: files created, tests written, lines of code

### Rule 3: JUDGE PREVIOUS STEP

Every agent evaluates memories from the previous step using `memory_judge`. This is NOT optional. Coder judges Planner's decisions. Reviewer judges Coder's patterns. This creates a distributed feedback chain.

- Score 0.8-1.0: Directly useful, applied the knowledge
- Score 0.5-0.7: Relevant context, good to know
- Score 0.2-0.4: Marginally relevant
- Score 0.0-0.1: Not useful or misleading

**Include explanation with every judgment.** "Score 0.8" alone is insufficient — explain WHY.

### Rule 4: JUDGE IGNORED SEARCH RESULTS

If you received search results but consciously ignored them — you MUST call `memory_judge` on those records with a low score and explain WHY they were not applicable. This teaches Engram what is irrelevant in specific contexts.

### Rule 5: ANTIPATTERNS ARE CRITICAL

If `memory_search` returns antipattern records — you MUST address them explicitly in your output. Either:
- Explain why your current approach is different and won't trigger the same issue
- Change your approach to avoid the antipattern

Never silently ignore antipatterns. This is a BLOCKING requirement.

### Rule 6: STORE FAILURES IMMEDIATELY

Failed approach, rejected plan, broken test, compilation error — store as `antipattern` or `bugfix` IMMEDIATELY before any other action. Do not wait until the end of the step. Failures are the most valuable memories.

**Required fields for failure records:**
- What was tried (exact code/approach)
- Why it failed (error message, root cause)
- What to do instead (alternative approach)
- Severity (blocking / degraded / cosmetic)

### Rule 7: USE CORRECT MEMORY TYPES

- `decision` — architectural choices, technology selections, approach decisions with reasoning
- `pattern` — code conventions, structures, recurring solutions that work
- `bugfix` — bugs found and fixed, with root cause and solution
- `context` — project state, baseline metrics, codebase analysis, routine observations
- `antipattern` — approaches that failed, with cost and alternative
- `insight` — derived knowledge from multiple observations (usually system-generated)

**Using the wrong type degrades search precision.** Think before choosing.

### Rule 8: USE CORRECT MODES

- `debug` — investigating bugs, reading logs, analyzing errors
- `architecture` — designing structure, choosing technology
- `coding` — writing code, implementing features
- `review` — reviewing code, checking quality
- `plan` — planning tasks, evaluating approaches
- `routine` — minor operations, baseline checks

**Mode MUST match agent role:** Planner uses `plan`/`architecture`. Coder uses `coding`. Reviewer uses `review`. Tester uses `debug`/`routine`.

### Rule 9: TAG EVERYTHING

Every `memory_store` MUST include relevant tags. Tags enable precise future search. **Minimum 3 tags per record.** Include:
- Phase identifier: `phase-1`, `phase-2`, etc.
- Module name: `crypto`, `store`, `net`, `sync`, `commands`, `frontend`, `cli`
- Technology: `ed25519`, `rusqlite`, `noise`, `svelte`, `tauri`, etc.
- Step name: `preflight`, `plan`, `coder`, `review`, etc.

### Rule 10: PARENT_ID FOR CHAINS

When current action is a consequence of a previous memory — set `parent_id`. Plan leads to implementation leads to bugfix — chain them. This builds causal graphs that enable Engram to derive insights.

**Mandatory chains:**
- PLAN record → CODER record (parent_id = plan memory id)
- CODER record → REVIEW finding (parent_id = coder memory id)
- REVIEW finding → BUGFIX (parent_id = review memory id)
- BUGFIX → subsequent CODER retry (parent_id = bugfix memory id)

### Rule 11: REACTIVE SEARCH (mid-work)

When an agent hits an error, unexpected behavior, or uncertainty DURING work — `memory_search` IMMEDIATELY before attempting a fix. Do NOT guess solutions — check Engram first.

**Triggers:**
- Compilation error → `memory_search query="[error message keywords]" mode=debug type=bugfix`
- Unexpected API behavior → `memory_search query="[crate/library] [function] unexpected" mode=debug type=antipattern`
- Uncertainty about approach → `memory_search query="[technology] [pattern] best practice" mode=coding type=pattern`

**If search returns relevant results:** apply them and `memory_judge` the record.
**If search returns nothing:** proceed with your fix, then `memory_store` the solution as `bugfix` so future agents benefit.

### Rule 12: DECISION STORE (mid-work)

When an agent faces a choice between two or more approaches — `memory_search` for precedents, then `memory_store` the decision with reasoning BEFORE implementing it.

**Format:**
```
memory_store
  type: decision
  context: "[What decision was needed]"
  action: "Options: [A] vs [B]. Searched Engram: [results or empty]. Chose [A] because [reasoning]."
  result: "Decision: [A]. Trade-off: [what we give up]. Revisit if: [conditions]."
  tags: [phase, module, technology, "decision"]
  mode: coding
```

**This applies to all agents**, not just Coder. Planner choosing between plan structures, Reviewer choosing severity — all decisions worth recording.

### Rule 13: DISCOVERY STORE (mid-work)

When an agent discovers something non-obvious DURING work — `memory_store` IMMEDIATELY. Do not wait until the end of the step.

**What counts as a discovery:**
- Crate API works differently than expected or documented
- Workaround needed for a dependency version
- Test revealed an edge case not described in spec
- Two modules interact in an unexpected way
- Performance characteristic worth noting

**Format:**
```
memory_store
  type: pattern (or antipattern if negative)
  context: "Discovery during [step]: [module/file]"
  action: "Found: [what]. Expected: [what]. Actual: [what]."
  result: "Implication: [impact]. Workaround: [if any]. Future agents should: [advice]."
  tags: [phase, module, technology, "discovery"]
  mode: [current mode]
```

**Do not filter discoveries by importance.** What seems trivial now may save hours in a future phase.

---

## ENGRAM INTERACTION MINIMUMS PER PIPELINE STEP

These are hard minimums. Exceeding them is encouraged.

| Step | memory_search | memory_store | memory_judge |
|---|---|---|---|
| PREFLIGHT | 2 | 1 | 0 (first step) |
| READ | 2 | 1 | judge previous phase |
| PLAN | 3 (per technology) | 1 | judge READ |
| MEMORY_CHECK(plan) | 1 per technology | 1 | judge PLAN |
| PLAN_REVIEW | 2 | 1 | judge MEMORY_CHECK |
| CODER | 3 per work unit + reactive | 1 per work unit + decisions + discoveries | judge PLAN |
| MEMORY_CHECK(code) | 1 per module | 1 | judge CODER |
| REVIEW (each) | 2 | 1 + decisions + discoveries | judge CODER |
| TEST | 1 | 1 | 0 |
| VERIFY | 2 | 1 | judge entire phase |
| COMMIT | 0 | 1 (final) | 0 |

---

## ENFORCEMENT — TEAM LEAD CHECKLIST

**Before approving EVERY step transition,** Team Lead MUST verify:

1. ☐ Agent called `memory_search` at least the minimum number of times
2. ☐ Agent called `memory_store` with meaningful content (not empty/placeholder)
3. ☐ Agent called `memory_judge` on previous step's records (where required)
4. ☐ Antipatterns from search results addressed explicitly
5. ☐ Ignored search results judged with low score + explanation
6. ☐ Memory type matches content (Rule 7)
7. ☐ Memory mode matches agent role (Rule 8)
8. ☐ Tags present, minimum 3, include phase number (Rule 9)
9. ☐ Parent_id set for chained records (Rule 10)

**If ANY check fails:**
1. STOP the pipeline
2. Return to the violating agent with specific violation description
3. Agent MUST fix the violation (make missing Engram calls)
4. Only then may the pipeline proceed

**Do NOT let agents skip Engram steps to "save time" or "because results were empty."**

---

## Project: GhostMesh

P2P messenger with BBS philosophy. Gossip sync, E2E encryption, zero servers.
Tauri 2 (Rust backend) + Svelte 5 (frontend). Full spec: `docs/spec-v02.md`

---

## Team Structure

```
Team Lead (you) — coordination, enforcement, commit, final judge
├── Planner (Explore) — context gathering, planning
├── Memory Checker (Explore) — deep Engram verification
├── Plan Reviewer (Explore) — plan quality review
├── Coder (Full) — test-first implementation
├── Reviewer Security (Explore) — security review
├── Reviewer Quality (Explore) — code quality review
├── Reviewer Coverage (Explore) — test coverage review
├── Tester (bash) — build, lint, tests
└── Verifier (Explore) — requirement verification
```

---

## Pipeline (per phase)

```
PREFLIGHT → READ → PLAN → MEMORY_CHECK(plan) → PLAN_REVIEW → CODER → MEMORY_CHECK(code) → REVIEW×3 → TEST → VERIFY → COMMIT
```

**Every step transition requires Team Lead approval + Engram compliance check.** If any step produces a BLOCKING issue — pipeline stops, returns to the appropriate earlier step.

**Maximum 3 fix loop iterations** between CODER and REVIEW. After 3 failures — escalate to Team Lead for manual decision.

---

## PIPELINE STEPS — DETAILED INSTRUCTIONS

### Step 1: PREFLIGHT

**Agent:** Team Lead (you, no teammate needed)

**Engram search (required):**
```
memory_search query="ghostmesh phase [N] [module]" mode=routine
memory_search query="ghostmesh build baseline" mode=routine
```

**Action:** Run git status, check entry criteria for current phase. Capture baseline.

**Engram store (required):**
```
memory_store
  type: context
  context: "Preflight for Phase [N]: [name]"
  action: "git status: [output]. Entry criteria: [list with met/unmet status]"
  result: "Baseline captured. Ready to proceed / Blocked by: [details]"
  tags: ["preflight", "phase-N", "ghostmesh", "baseline"]
  mode: routine
```

---

### Step 2: READ

**Agent:** Planner teammate (Explore)

**Prompt template:**
> You are the Planner for GhostMesh Phase [N]: [name]. Read the relevant spec sections: [§list]. Read existing code from previous phases. Gather context for planning. Follow the Engram protocol in CLAUDE.md strictly — this is non-negotiable.

**Engram search (required):**
```
memory_search query="ghostmesh [module] architecture" mode=architecture
memory_search query="ghostmesh phase [N-1] results" mode=routine
```

**Engram judge:** Judge previous phase's final commit record.

**Engram store (required):**
```
memory_store
  type: context
  context: "Spec analysis for Phase [N]: [name]"
  action: "Read spec §[sections]. Analyzed [N] types/structs. Mapped interfaces with Phase [N-1]."
  result: "Modules: [list]. Dependencies: [chain]. Key interfaces: [list]. Ready for PLAN."
  tags: ["read", "phase-N", "ghostmesh", module-tags]
  mode: architecture
```

---

### Step 3: PLAN

**Agent:** Planner teammate (continues)

**Engram search (required — one per technology/module in this phase):**

Phase-specific search templates:

**Phase 2 (Crypto):**
```
memory_search query="ed25519 dalek identity key generation" mode=plan type=pattern
memory_search query="chacha20 poly1305 encryption nonce" mode=plan type=bugfix
memory_search query="noise protocol snow crate handshake" mode=plan type=antipattern
memory_search query="x25519 diffie hellman hkdf" mode=plan type=pattern
memory_search query="argon2id aes-gcm key storage" mode=plan type=bugfix
```

**Phase 3 (Store):**
```
memory_search query="rusqlite sqlite schema migration" mode=plan type=pattern
memory_search query="rusqlite spawn_blocking async" mode=plan type=bugfix
memory_search query="sqlite cbor blob storage" mode=plan type=antipattern
```

**Phase 4 (Net):**
```
memory_search query="tokio tcp noise transport" mode=plan type=pattern
memory_search query="mdns-sd rust discovery" mode=plan type=bugfix
memory_search query="cbor wire protocol framing" mode=plan type=pattern
memory_search query="peer connection management reconnect" mode=plan type=antipattern
```

**Phase 5 (Sync):**
```
memory_search query="lamport clock logical ordering" mode=plan type=pattern
memory_search query="frontier diff sync algorithm" mode=plan type=decision
memory_search query="gossip protocol sync engine" mode=plan type=antipattern
```

**Phase 6 (Tauri):**
```
memory_search query="tauri 2 command registration state" mode=plan type=pattern
memory_search query="tauri event emit async" mode=plan type=bugfix
memory_search query="tauri ipc boundary security" mode=plan type=antipattern
```

**Phase 7 (Frontend):**
```
memory_search query="svelte 5 runes state derived" mode=plan type=pattern
memory_search query="tauri invoke listen frontend" mode=plan type=bugfix
memory_search query="svelte store reactivity" mode=plan type=antipattern
```

**Phase 8 (CLI):**
```
memory_search query="clap rust cli commands" mode=plan type=pattern
memory_search query="core library reuse cli binary" mode=plan type=pattern
```

**Engram store (required):**
```
memory_store
  type: decision
  context: "Plan for Phase [N]: [name]"
  action: "[K] work units planned. Dependencies: [chain]. Estimated files: [N]."
  result: "Plan approved/pending. Critical path: [path]. Risks: [list]."
  tags: ["plan", "phase-N", "ghostmesh", module-tags]
  mode: plan
```

---

### Step 4: MEMORY_CHECK (plan)

**Agent:** Memory Checker teammate (Explore)

**Prompt template:**
> You are the Memory Checker. Verify Phase [N] plan against Engram. You MUST search for EACH technology and module used in this phase — one search per technology. Check for antipatterns, bugfixes, known issues. Your job is to find relevant memories that the Planner may have missed. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required — one per technology):**
Use the phase-specific search templates from Step 3 above, but with `type=antipattern` and `type=bugfix` focus.

**Engram judge:** Judge Planner's decision record. Score and explain.

**Engram store (required):**
```
memory_store
  type: context
  context: "Memory check for Phase [N] plan"
  action: "Searched [N] queries across [technologies]. Found: [N relevant / N antipatterns / N bugfixes]"
  result: "Report: [findings]. BLOCKING: [list or none]. Recommendations: [list]"
  tags: ["memory-check", "phase-N", "ghostmesh", technology-tags]
  mode: review
```

---

### Step 5: PLAN_REVIEW

**Agent:** Plan Reviewer teammate (Explore)

**Prompt template:**
> You are the Plan Reviewer. Review Phase [N] plan + Memory Checker report. Verify completeness vs spec sections [§list], dependency on previous phases, types/interfaces match, all work units present. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required):**
```
memory_search query="plan review quality completeness phase" mode=review
memory_search query="ghostmesh phase [N] [module] spec compliance" mode=review
```

**Engram judge:** Judge Memory Checker's report. Score and explain.

**Engram store (required):**
```
memory_store
  type: decision
  context: "Plan review for Phase [N]. [K] criteria checked."
  action: "Evaluated: completeness vs spec, dependency order, interface consistency, Engram findings."
  result: "Verdict: [approved/rejected]. Details: [per-criterion]. Issues: [list or none]."
  tags: ["plan-review", "phase-N", "ghostmesh"]
  mode: review
```

---

### Step 6: CODER

**Agent:** Coder teammate (Full permissions)

**Prompt template:**
> You are the Coder. Implement Phase [N]: [name]. Work units [list] sequentially. Test-first: write tests BEFORE implementation for each module. You MUST search Engram BEFORE starting each work unit and store results AFTER completing each work unit. Read docs/spec-v02.md sections [§list] for exact structures. Follow the Engram protocol in CLAUDE.md strictly — this is non-negotiable.

**Engram search (required — BEFORE EACH work unit):**
```
memory_search query="[module] [technology] implementation" mode=coding type=pattern
memory_search query="[module] [technology] known issues" mode=coding type=bugfix
memory_search query="[module] test patterns edge cases" mode=coding type=pattern
```

**Engram judge:** Judge Planner's plan record at start. Score and explain how it guided implementation.

**Engram store (required — AFTER EACH work unit):**
```
memory_store
  type: pattern
  context: "Phase [N], unit [K]: [file_path]"
  action: "Implemented [description]. Tests: [N written]. Key decisions: [list with reasoning]."
  result: "Files: [list]. Lines: ~[N]. Tests: [pass/fail]. Patterns used: [list]."
  tags: ["implementation", "phase-N", "ghostmesh", module-tags, technology-tags]
  mode: coding
  parent_id: [plan memory id]
```

**On failure (required — IMMEDIATELY):**
```
memory_store
  type: bugfix (or antipattern)
  context: "Phase [N], unit [K]: [file_path] — FAILURE"
  action: "Tried: [approach]. Error: [exact message]."
  result: "Root cause: [analysis]. Fix: [what worked]. Avoid: [what to not do]."
  tags: ["bugfix", "phase-N", module-tags, technology-tags]
  mode: debug
```

**Universal Coder rules:**
1. Test first: `#[cfg(test)] mod tests` BEFORE implementation
2. `core/` is Tauri-free. Never `use tauri::` inside core
3. Keys stay in Rust. IPC types expose `peer_id: String`, never raw secrets
4. CBOR for wire. JSON for IPC (Tauri default)
5. All core functions return `Result<T, CoreError>`. Commands convert to `String`
6. Async: net/sync use `tokio`. Store: sync rusqlite via `spawn_blocking`

---

### Step 7: MEMORY_CHECK (code)

**Agent:** Memory Checker teammate (Explore)

**Prompt template:**
> You are the Memory Checker. Review Phase [N] code implementation. Search Engram for each module implemented — check for known bugfixes, antipatterns, correctness issues. Your job is to find problems the Coder may have repeated despite Engram warnings. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required — per module implemented):**

Phase-specific code review searches:

**Phase 2:** `crypto identity ed25519 x25519`, `chacha20 nonce reuse`, `noise handshake order`, `argon2 params`, `key zeroing zeroize`
**Phase 3:** `sqlite schema migration version`, `rusqlite error handling`, `cbor parent_ids encoding`, `outbox delivery tracking`
**Phase 4:** `noise transport tcp`, `wire framing length prefix`, `mdns discovery registration`, `peer reconnect backoff`
**Phase 5:** `lamport clock overflow`, `frontier merge conflict`, `sync deadlock`, `outbox cleanup race`
**Phase 6:** `tauri command state`, `ipc key leak`, `event emit error`, `csp configuration`
**Phase 7:** `svelte reactivity memory leak`, `invoke error handling`, `event listener cleanup`

**Engram judge:** Judge ALL of Coder's records for this phase. Score each and explain.

**Engram store (required):**
```
memory_store
  type: context
  context: "Code memory check for Phase [N]"
  action: "Searched [N] queries. Reviewed [N] Coder records. Cross-referenced with known issues."
  result: "Findings: [list]. BLOCKING: [list or none]. Coder compliance: [assessment]."
  tags: ["memory-check", "code-review", "phase-N", "ghostmesh"]
  mode: review
```

---

### Step 8: REVIEW ×3 (parallel)

#### Reviewer Security (Explore)

> You are the Security Reviewer for GhostMesh Phase [N]. Focus: [phase-specific security concerns]. You MUST search Engram for security-related antipatterns and bugfixes before reviewing. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required):**
```
memory_search query="ghostmesh security [module] vulnerability" mode=review type=antipattern
memory_search query="[technology] security best practice" mode=review type=pattern
```

**Phase-specific security checks:**

Phase 2: Private keys zeroed on drop. Ed25519 signs `(header ‖ ciphertext ‖ nonce)`. ChaCha20 nonces random, never reused. X25519 DH + HKDF correct salt/info. Argon2id params reasonable.
Phase 3: Encrypted keys stored correctly. SQL injection impossible (parameterized queries). No plaintext secrets in DB.
Phase 4: Noise_XX pattern correct (3 messages). AuthHello AFTER encrypted channel. No plaintext before handshake. Length prefix validated (max 256 KiB).
Phase 5: Frontier manipulation resistance. Message ID verification. Signature check before processing.
Phase 6: Private keys NEVER cross IPC. CSP: `default-src 'self'`. All commands in capabilities. No eval/dynamic scripts.
Phase 7: No sensitive data in DOM/console. Event listeners cleaned up. Input sanitized.

**Engram judge:** Judge Coder's records. Score and explain.

**Engram store (required):**
```
memory_store
  type: context (or antipattern if issues found)
  context: "Security review for Phase [N]"
  action: "Checked [N] security criteria. Searched Engram for known vulnerabilities."
  result: "Verdict: [pass/fail]. Issues: [list with severity]. BLOCKING: [list or none]."
  tags: ["security-review", "phase-N", "ghostmesh", module-tags]
  mode: review
```

#### Reviewer Quality (Explore)

> You are the Quality Reviewer for Phase [N]. Check: error handling, ownership, module boundaries, interface consistency with previous phases, code conventions. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search, judge, store:** Same structure as Security Reviewer, with quality-focused queries.

#### Reviewer Coverage (Explore)

> You are the Coverage Reviewer for Phase [N]. Check: tests for every public function, roundtrip tests, edge cases, error paths tested. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search, judge, store:** Same structure, with test-coverage-focused queries.

**If issues found:** Fix loop → CODER → MEMORY_CHECK → REVIEW. Max 3 iterations. Each iteration MUST produce Engram records (the fix, the re-review finding).

---

### Step 9: TEST

**Agent:** Tester teammate (bash)

> You are the Tester for Phase [N]. Run: [phase-specific test commands]. Report results. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required):**
```
memory_search query="ghostmesh phase [N] test build" mode=routine type=bugfix
```

**Engram store (required):**
```
memory_store
  type: context (or bugfix if failures)
  context: "Test run for Phase [N]"
  action: "Ran: [commands]. Duration: [time]."
  result: "Tests: [N pass / N fail]. Build: [success/failure]. Errors: [list or none]."
  tags: ["test", "phase-N", "ghostmesh", "build"]
  mode: routine
```

---

### Step 10: VERIFY

**Agent:** Verifier teammate (Explore)

> You are the Verifier for Phase [N]. Check implementation against spec sections [§list]. Verify ALL exit criteria. This is completeness verification — every requirement from the spec for this phase must be accounted for. Follow the Engram protocol in CLAUDE.md strictly.

**Engram search (required):**
```
memory_search query="ghostmesh phase [N] requirements completeness" mode=review
memory_search query="ghostmesh spec compliance [module]" mode=review
```

**Engram judge:** Judge the entire phase's memory chain — from PLAN through CODER through REVIEW.

**Engram store (required):**
```
memory_store
  type: context
  context: "Verification for Phase [N]"
  action: "Checked [N] requirements from spec §[sections]. Verified exit criteria."
  result: "Verdict: [complete/incomplete]. Coverage: [N/M requirements]. Missing: [list or none]."
  tags: ["verify", "phase-N", "ghostmesh", "spec-compliance"]
  mode: review
```

---

### Step 11: COMMIT

**Agent:** Team Lead. Stage, commit with message `phase-N: [description]`.

**Engram store (required — final phase record):**
```
memory_store
  type: decision
  context: "Phase [N] complete: [name]"
  action: "Committed. Files: [N]. Tests: [N]. Engram records this phase: [N]."
  result: "Phase [N] done. Artifacts: [list]. Next phase entry criteria: [met/unmet for Phase N+1]."
  tags: ["commit", "phase-N", "ghostmesh", "milestone"]
  mode: routine
```

---

## PHASES — EXECUTION ORDER

Each phase is an independent pipeline run. Phases MUST be executed in order — each depends on artifacts from previous phases.

### Phase 1: Scaffold

**Scope:** Project structure, configs, shared types. No business logic.
**Agent:** Team Lead only (no Coder needed).
**Spec sections:** §3 (architecture), §12 (stack), §13 (project structure)

**Work units:**
- A1. `cargo tauri init`, create all directories per §13
- A2. `Cargo.toml` with all dependencies from §12.1
- A3. `package.json`, `vite.config.ts`, `svelte.config.js`, `tsconfig.json` from §12.2
- A4. `tauri.conf.json`, `capabilities/default.json`
- A5. `core/types.rs` — all shared domain types from §4–§9
- A6. `types.rs` — IPC DTO types from §3.3, §3.4

**Entry criteria:** Empty repo or clean git state.
**Exit criteria:** `cargo check` passes. `npm install` succeeds. All directories exist.
**Pipeline:** PREFLIGHT → scaffold → TEST → COMMIT (simplified)

---

### Phase 2: Crypto

**Scope:** All cryptographic primitives. Pure Rust, no Tauri dependency.
**Spec sections:** §4 (identity), §10 (crypto)

**Work units:**
- B1. `core/crypto/identity.rs` — Ed25519 + X25519 keypair generation, peer_id derivation
- B2. `core/crypto/sign.rs` — Ed25519 sign/verify
- B3. `core/crypto/encrypt.rs` — ChaCha20-Poly1305 encrypt/decrypt, Argon2id + AES-256-GCM for key storage
- B4. `core/crypto/exchange.rs` — X25519 DH + HKDF key derivation
- B5. `core/crypto/noise.rs` — Noise_XX wrapper via `snow`
- B6. `core/crypto/mod.rs` — public API

**Entry criteria:** Phase 1 complete. `cargo check` passes.
**Exit criteria:** `cargo test -p ghostmesh -- crypto` — all pass. Roundtrip tests for every primitive.

**Coder rules:**
- Test first: `#[cfg(test)] mod tests` BEFORE implementation
- No `use tauri::` in `core/`
- All functions return `Result<T, CryptoError>`
- Random bytes via `rand::rngs::OsRng`

**Security review focus:**
- Ed25519 signs `(header ‖ ciphertext ‖ nonce)`
- ChaCha20 nonces random, never reused
- X25519 DH + HKDF correct salt/info per §10.2
- Argon2id params: memory=64MB, iterations=3, parallelism=4
- Private keys zeroed on drop (zeroize)

---

### Phase 3: Store

**Scope:** SQLite storage layer. Pure Rust, no Tauri dependency.
**Spec sections:** §7 (store)

**Work units:**
- C1. `core/store/db.rs` — SQLite init, schema creation (all 9 tables from §7.1), connection pool
- C2. `core/store/chats.rs` — CRUD for chats, chat_members, chat_keys
- C3. `core/store/messages.rs` — insert/query messages, frontiers update
- C4. `core/store/outbox.rs` — outbox CRUD, delivery tracking
- C5. `core/store/mod.rs` — Store struct, public API

**Entry criteria:** Phase 2 complete. Crypto types available.
**Exit criteria:** `cargo test -p ghostmesh -- store` — all pass. CRUD roundtrips for all 9 tables.

**Coder rules:**
- rusqlite via `spawn_blocking` for async compatibility
- All 9 tables from §7.1 — exact schema match
- CBOR for `parent_ids` column
- Encrypted keys stored via Phase 2 crypto
- Storage policy from §7.2 (sync_log ring buffer, peer_addresses cleanup)

---

### Phase 4: Net

**Scope:** Network transport, wire protocol, peer discovery. Pure Rust, no Tauri dependency.
**Spec sections:** §8 (net)

**Work units:**
- D1. `core/net/wire.rs` — `WireMessage` enum (all 8 variants from §8.5), CBOR encode/decode, length-prefixed framing
- D2. `core/net/transport.rs` — TCP + Noise_XX handshake per §8.2, encrypted read/write
- D3. `core/net/discovery.rs` — mDNS via `mdns-sd`, service `_ghostmesh._tcp.local`
- D4. `core/net/peer_manager.rs` — connection tracking, reconnect logic, peer address management
- D5. `core/net/mod.rs` — NetLayer struct, public API

**Entry criteria:** Phase 3 complete. Store available for peer_addresses.
**Exit criteria:** `cargo test -p ghostmesh -- net` — all pass. Wire encode/decode for ALL 8 variants. Transport roundtrip test.

**Coder rules:**
- `tokio` for async networking
- Noise_XX_25519_ChaChaPoly_BLAKE2b via `snow`
- Wire frames: 4-byte length prefix + CBOR payload (max 256 KiB)
- Port 9473 default
- PeerExchange gossip for address sharing

**Security review focus:**
- Noise_XX pattern correct (3 messages)
- AuthHello sent AFTER encrypted channel established
- No plaintext data before handshake complete
- Length prefix validated (max 256 KiB)

---

### Phase 5: Sync

**Scope:** Synchronization protocol, Lamport clocks, frontier logic. Pure Rust.
**Spec sections:** §6 (messages), §9 (sync)

**Work units:**
- E1. `core/sync/lamport.rs` — Lamport clock logic per §6.4
- E2. `core/sync/frontier.rs` — FrontierEntry, diff calculation, merge
- E3. `core/sync/engine.rs` — SyncEngine: full sync algorithm from §9.1, continuous sync per §9.3
- E4. `core/mod.rs` — Core aggregator struct tying crypto + store + net + sync

**Entry criteria:** Phase 4 complete. Net + Store + Crypto available.
**Exit criteria:** `cargo test -p ghostmesh -- sync` — all pass. Sync scenarios: empty frontier, partial overlap, full sync, offline peers.

**Coder rules:**
- Lamport: `max(local, remote) + 1` — both send and receive
- Message ordering: `(lamport_ts, author_peer_id)` — deterministic
- Continuous sync: re-sync every 60s, PeerExchange every 5min
- Outbox cleanup after SyncAck

---

### Phase 6: Tauri Integration

**Scope:** Tauri commands, events, state management. Wires core to frontend.
**Spec sections:** §3.3 (commands), §3.4 (events), §14 (tauri features)

**Work units:**
- F1. `state.rs` — AppState struct wrapping Core
- F2. `events.rs` — typed emit helpers for all 9 events from §3.4
- F3. `commands/identity.rs` — create_identity, get_identity, export_identity, import_identity
- F4. `commands/chats.rs` — create_chat, list_chats, get_chat, generate_invite, join_chat, leave_chat
- F5. `commands/messages.rs` — send_message, get_messages, get_message_detail
- F6. `commands/network.rs` — get_peers, get_connections, get_outbox, add_manual_peer, get_sync_log
- F7. `commands/settings.rs` — get_settings, update_settings
- F8. `main.rs` — Builder, command registration, tray setup, background tasks

**Entry criteria:** Phase 5 complete. Full Core struct available.
**Exit criteria:** `cargo build -p ghostmesh` succeeds. All 18 commands registered. All 9 events emitted.

**Coder rules:**
- Commands are THIN wrappers: `state.core.method()` → emit event → return DTO
- Private keys NEVER in command return types (peer_id: String, not raw keys)
- Errors: `CoreError` → `String` for IPC
- Background tasks via `tauri::async_runtime::spawn`
- System tray: green/yellow/grey per §14.1

**Security review focus:**
- No private key data crosses IPC boundary
- CSP: `default-src 'self'`
- All commands listed in `capabilities/default.json`
- No `eval()` or dynamic script loading

---

### Phase 7: Frontend

**Scope:** Svelte UI — all screens from spec.
**Spec sections:** §3.5 (svelte usage), §11 (UX/screens)

**Work units:**
- G1. `styles/global.css` — CSS variables, dark theme, monospace for tech info
- G2. `lib/api.ts` — typed wrappers for all 18 `invoke()` calls
- G3. `lib/events.ts` — typed wrappers for all 9 `listen()` subscriptions
- G4. `lib/stores/*.ts` — reactive stores: identity, chats, messages, network
- G5. `components/Sidebar.svelte` — chat list, online/total indicators
- G6. `components/ChatView.svelte` — message feed, lamport numbering, delivery status
- G7. `components/MessageRow.svelte` — single message with metadata
- G8. `components/InspectorPanel.svelte` — packet details, members, outbox
- G9. `components/NetworkDashboard.svelte` — metrics, connections, sync log
- G10. `components/InviteDialog.svelte` — generate/paste invite, QR
- G11. `components/Settings.svelte` — identity, network, storage settings
- G12. `components/PeerIndicator.svelte` — connection status badge
- G13. `App.svelte` + `main.ts` — routing, layout, status bar

**Entry criteria:** Phase 6 complete. All IPC commands and events available.
**Exit criteria:** `npm run build` succeeds. All 6 screens from §11.2 present.

**Coder rules:**
- Svelte 5 runes: `$state`, `$derived`, `$effect`
- Stores init from `invoke()` on mount, update from `listen()` events
- No business logic in frontend — display + input only
- Delivery status: `[queued]` / `[N/M]` / `[ALL]` per §11.3

---

### Phase 8: CLI

**Scope:** Dev CLI reusing core.
**Spec sections:** §3.2 (CLI description), §16 Phase 1 (CLI commands)

**Work units:**
- H1. `cli/Cargo.toml` — dependency on core via path
- H2. `cli/src/main.rs` — clap commands: `send`, `read`, `peers`, `sync`, `identity`

**Entry criteria:** Phase 5 complete. Core struct available.
**Exit criteria:** `cargo build -p ghostmesh-cli` succeeds.
**Pipeline:** PREFLIGHT → READ → PLAN → CODER → TEST → COMMIT (lightweight)

---

### Phase 9: Integration

**Scope:** Full build, cross-module tests, smoke test.
**Spec sections:** All

**Work units:**
- I1. `cargo build` — full project
- I2. `cargo test` — all tests pass
- I3. `npm run build` — frontend compiles
- I4. `cargo tauri dev` — app launches, basic flow works

**Entry criteria:** Phases 1–8 complete.
**Exit criteria:** All builds pass. Tauri dev launches without errors.
**Pipeline:** PREFLIGHT → TEST → VERIFY → COMMIT

---

## RUNNING A PHASE

### Launch format

```
Phase: [N] [name]
Pipeline: [steps for this phase]
Current step: [step]
Engram records this step: [count]
Status: [status]
```

### Between phases

Engram bridges context. At the start of each new phase:
1. `memory_search` for previous phase results, antipatterns, bugfixes
2. `git log --oneline -10` to see what was committed
3. Verify entry criteria before proceeding

### Launching a specific phase

User says: `Phase 2` or `Phase 2: Crypto` → run full pipeline for that phase only.
User says: `Phases 2-5` → run phases 2, 3, 4, 5 sequentially, committing after each.
User says: `Continue` → detect current state from git + Engram, resume next phase.

---

## ERROR HANDLING

- **Engram unavailable:** STOP and report to user. Do NOT continue without Engram.
- **Empty search results:** Normal. Continue. Store results anyway.
- **Conflicting results:** Store both. Reviewer decides. Record the conflict.
- **Agent forgets Engram:** Team Lead catches at enforcement checklist. Return to agent. Pipeline blocked until Engram calls are made.
- **Phase entry criteria unmet:** STOP. Complete previous phase first.

---

## GHOSTMESH PROJECT REFERENCE

### Architecture

```
ghost-mesh/
├── src-tauri/src/
│   ├── main.rs, state.rs, events.rs, types.rs
│   ├── commands/{mod,identity,chats,messages,network,settings}.rs
│   └── core/
│       ├── types.rs
│       ├── crypto/{mod,identity,sign,encrypt,exchange,noise}.rs
│       ├── store/{mod,db,chats,messages,outbox}.rs
│       ├── net/{mod,wire,transport,discovery,peer_manager}.rs
│       └── sync/{mod,lamport,frontier,engine}.rs
├── src/
│   ├── App.svelte, main.ts
│   ├── lib/{api,events}.ts, lib/stores/{identity,chats,messages,network}.ts
│   ├── components/{Sidebar,ChatView,MessageRow,InspectorPanel,NetworkDashboard,
│   │   PeerIndicator,InviteDialog,OnboardingScreen,Settings}.svelte
│   └── styles/global.css
├── cli/src/main.rs
└── docs/spec-v02.md
```

### Critical rules

1. `core/` MUST NOT depend on `tauri`.
2. `commands/` are THIN wrappers only.
3. Private keys NEVER cross IPC.
4. Wire = CBOR. IPC = JSON.
5. Every core module has unit tests.
6. Rust 2021. Svelte 5 runes.

### IPC Commands (18 total)

```
create_identity, get_identity, export_identity, import_identity,
create_chat, list_chats, get_chat, generate_invite, join_chat, leave_chat,
send_message, get_messages, get_message_detail,
get_peers, get_connections, get_outbox, add_manual_peer, get_sync_log,
get_settings, update_settings
```

### IPC Events (9 total)

```
message:new, peer:connected, peer:disconnected,
sync:progress, sync:complete, delivery:ack,
network:status, chat:member_joined, chat:member_left
```

### Rust deps

```
tauri 2, tauri-plugin-{dialog,clipboard-manager,notification} 2,
ed25519-dalek 2, x25519-dalek 2, chacha20poly1305 0.10, blake2 0.10,
hkdf 0.12, argon2 0.5, aes-gcm 0.10, snow 0.9, rand 0.8,
tokio 1 (full), rusqlite 0.31 (bundled), mdns-sd 0.11,
serde 1, serde_json 1, ciborium 0.2, thiserror 1, tracing 0.1,
uuid 1, hex 0.4, base62 2, clap 4 (cli)
```

### Frontend deps

```
@tauri-apps/api 2, @tauri-apps/plugin-{dialog,clipboard-manager,notification} 2, qrcode 1.5
```
