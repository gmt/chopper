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

**Decision (superseded):** Preserve legacy single-line alias support while introducing TOML.

**Why (historical):** Enabled incremental migration without forcing a flag-day rewrite.

**Current state:** Legacy one-line alias support has been removed; runtime discovery is TOML-only.

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

For disable toggles specifically, truthy-token parsing is intentionally
ASCII-based (`1|true|yes|on`, ASCII case-insensitive) so non-ASCII lookalikes remain
unknown and do not accidentally disable features.

---

## 8) Docs split by audience

**Decision:** Keep root README brief and route detailed semantics to `doc/`.

**Why:** Reduces onboarding friction and prevents quickstart docs from becoming
an overwhelming spec dump.

---

## 9) User-scoped journal namespaces as new default

**Decision:** `journal.user_scope` defaults to `true`. User-scoped namespace
derivation (`u<uid>-<username>-<namespace>`) is the default for all new
`[journal]` configurations. Literal passthrough requires explicit
`user_scope = false`.

**Why:** Anti-collision and security measure — prevents users from
accidentally writing to each other's namespaces.

---

## 10) D-Bus broker instead of CLI subprocess

**Decision:** The journal namespace broker (`chopper-journal-broker`) is a
D-Bus system service called via `com.chopperproject.JournalBroker1`, not a
CLI subprocess.

**Why:** D-Bus provides credential-based UID verification, polkit integration,
and bus-activation. This is the standard pattern for privileged system services
on Linux (c.f. `systemd-hostnamed`, `systemd-timedated`).

---

## 11) Journal policy fields are client-side with broker-side enforcement

**Decision:** Per-alias journal policy fields (`max_use`,
`rate_limit_interval_usec`, `rate_limit_burst`) are specified in the alias
TOML config and passed to the broker via D-Bus. The broker enforces hard
server-side limits.

**Why:** Allows per-alias customization while preventing abuse.
