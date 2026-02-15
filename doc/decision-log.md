# chopper decision log

High-level implementation bets and why they were chosen.

---

## 1) Per-alias files over monolithic config

**Decision:** Prefer one file per alias as primary UX.

**Why:** Keeps creation/maintenance simple, avoids loading/editing giant
configuration blobs for common workflows.

---

## 2) TOML as concrete DSL surface

**Decision:** Make TOML the declarative format for first-class aliases.

**Why:** Human-readable, easy to diff/review, mature parser ecosystem, low
syntax surprise.

---

## 3) Keep legacy one-line aliases

**Decision:** Preserve legacy single-line alias support while introducing TOML.

**Why:** Enables incremental migration without forcing a flag-day rewrite.

---

## 4) Automatic transparent caching

**Decision:** Cache normalized manifests automatically with metadata/fingerprint
invalidation and self-healing behavior.

**Why:** Improves runtime performance for simple aliases while requiring no
extra user workflow.

---

## 5) Reconcile is optional

**Decision:** Rhai runtime reconcile path is opt-in per alias.

**Why:** Keeps common aliases fast/simple while still supporting advanced
runtime mutation where needed.

---

## 6) Journald integration is explicit and scoped

**Decision:** Route stderr to journald namespace only via explicit `[journal]`
config.

**Why:** Avoids global side effects while allowing strong observability for
selected aliases.

---

## 7) Reject only structurally unsafe strings

**Decision:** Validation rejects NUL/blank-required-field/unsafe-structural
cases, but preserves useful symbolic/path-like shapes.

**Why:** Balances safety with practical ergonomics; avoids over-restricting real
shell/path workflows.

---

## 8) Docs split by audience

**Decision:** Keep root README brief and route detailed semantics to `doc/`.

**Why:** Reduces onboarding friction and prevents quickstart docs from becoming
an overwhelming spec dump.
