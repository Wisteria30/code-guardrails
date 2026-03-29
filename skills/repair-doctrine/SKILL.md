---
name: repair-doctrine
description: >
  Repair doctrine for fixing policy violations (fallbacks, test-doubles).
  Teaches owner-layer identification, legal remedies, forbidden moves, and
  proof requirements. Use when encountering code-guardrails violations, when
  a hook warns or blocks an edit, when unsure how to fix a fallback or
  test-double, or when designing multi-file refactors to eliminate unauthorized
  defaults. This skill should trigger whenever violations, fallbacks,
  mock/stub/fake in production, policy-approved patterns, or repair strategy
  are discussed.
effort: high
---

# Repair Doctrine

This doctrine teaches you how to fix code-guardrails violations. It is not a
list of banned patterns — it is a framework for understanding **who owns the
decision** that a fallback or test-double implies, and **how to make that
ownership explicit**.

## Why pattern bans fail

A fallback like `.get("timeout", 30)` is not a syntax problem. It is an
unauthorized totalization: the code author chose a default value (`30`) that
the specification never authorized. Banning `.get(key, default)` only forces
a rewrite to `d["k"] if "k" in d else default` — same unauthorized decision,
different syntax.

The same applies to test doubles. Renaming `mock_client` to `secondary_client`
does not change that the runtime graph depends on a test-only implementation.

**Renaming and syntax-equivalent rewrites are not fixes.** They evade detection
without addressing ownership. The following are all equally wrong:

| Before (detected) | After (evaded) | Why it is the same |
|-|-|-|
| `d.get("k", "default")` | `d["k"] if "k" in d else "default"` | Same unauthorized δ: 1→A |
| `getattr(obj, "a", None)` | `obj.a if hasattr(obj, "a") else None` | Same unauthorized δ |
| `x = a or b` | `x = a if a else b` | Same unauthorized δ |
| `mock_client` | `secondary_client` | Same test-only implementation in runtime graph |

## Core insight

A fallback is a map `δ: 1 → A` (for Option) or `h: E → A` (for Result) that
collapses absence/failure into a concrete value. This map is **not derivable
from the type** — it requires a specification-level decision about what the
correct default is. When code contains an unauthorized fallback, the question
is not "how do I rewrite this line?" but "who owns this decision?"

## Owner layer identification

Every violation lives in a layer. The correct fix depends on which layer
**should** own the decision:

### Boundary
**Owns**: parse, validate, normalize.
Raw JSON, raw dict, raw env, raw queue payload stop here. After the boundary,
all data is typed and validated. Core code never touches raw inputs.

→ If a fallback exists because the data might be missing, the fix is to
**parse at the boundary** so downstream code receives a guaranteed type.

### Domain
**Owns**: invariants, always-valid state.
Entities and value objects maintain their own invariants through constructors
and methods. No global defaults. No partial construction.

→ If a fallback exists because the object might be in an invalid state, the
fix is to **make the domain always-valid** through construction-time validation.

### Application
**Owns**: orchestration.
Receives explicit errors or unions from the boundary and domain layers.
Decides where to translate errors, but never silently defaults.

→ If a fallback exists in the application layer, the fix is to **propagate
the error explicitly** as a typed exception or Result.

### Infrastructure
**Owns**: resilience policy.
Retry, cache, secondary sources, degrade mode are legitimate here — but only
as **designed policies** with metrics, TTL, and observability. Not as
`.catch(() => null)`.

→ If a fallback exists because of network/service failure, the fix is to
**elevate it to a resilience adapter** with proper policy.

### Composition root
**Owns**: implementation choice.
The only place where concrete implementations are selected and injected.
No mid-flow `FakeRepository()`.

→ If a test-double exists in production code, the fix is to **move the
selection to the composition root** and ensure the substitute is a
first-class adapter with contract tests.

### Test
**Owns**: test doubles.
The only place where partial implementations (mocks, stubs, fakes) are
legitimate. Even here, shared contract tests ensure doubles preserve the
port's laws.

→ If a test-double exists in production, the fix is to **move it to tests**
or **promote it to a first-class adapter** with a contract suite.

## Seven legal remedies

When a violation fires, choose **exactly one** of these remedies:

### 1. Approved policy default (blessed API)

The spec explicitly defines a default value. Make the approval visible:
```python
# policy-approved: REQ-123 locale default is defined by spec
lang = payload.get("lang", "ja-JP")
```
Ideally, close this in a dedicated API so the approval is structural, not a
comment: `LocalePolicy.default_locale()`.

### 2. Boundary parse

The data arrives untyped. Parse and validate at the boundary so core code
operates on guaranteed types.

For details, see [references/remedies-python.md](references/remedies-python.md)
and [references/remedies-typescript.md](references/remedies-typescript.md).

### 3. Optional/union + exhaustive handling

The value is genuinely optional. Keep it as `Optional[T]` or `T | null` and
force callers to handle both cases exhaustively. Never collapse it to a
default — let the caller decide.

### 4. Typed exception / contract violation

The state should be unreachable. Fail explicitly with a typed exception or
contract check. The outermost layer maps these to HTTP 4xx/5xx or CLI exit
codes.

### 5. Resilience adapter

Network/service failure requires a recovery strategy. Elevate to an
infrastructure-layer adapter with retry policy, cache TTL, metrics, and
circuit breaker. Not `.catch(() => null)`.

### 6. Move double to tests

Test doubles (mocks, stubs, fakes) belong in test files. If production code uses
a test double, move it to the test directory and use dependency injection to
provide the real implementation in production.

### 7. Promote substitute to first-class adapter + contract tests

If a production alternate implementation is genuinely needed (e.g., sandbox
payment gateway, in-memory event store for local dev), promote it from a fake
to a first-class adapter. This means:
- Rename to reflect its real purpose (not `FakeRepo` but `InMemoryRepo`)
- Register in composition root
- Share a contract test suite with all other implementations of the same port
- For stateful ports, use property-based or stateful tests

## Decision tree

```
Violation detected
├─ Already has policy-approved comment with REQ/ADR/SPEC reference?
│  └─ Yes → Already approved. Skip.
└─ No → Identify the owner layer
   ├─ Data arrives untyped (raw JSON, env, dict)?
   │  └─ Boundary parse (remedy 2)
   ├─ Value is genuinely optional per spec?
   │  └─ Optional/union + exhaustive handling (remedy 3)
   ├─ Spec defines an explicit default?
   │  └─ Approved policy default (remedy 1)
   ├─ State should be unreachable?
   │  └─ Typed exception / contract violation (remedy 4)
   ├─ External service failure?
   │  └─ Resilience adapter (remedy 5)
   └─ Test-double in production?
      ├─ Needed as alternate implementation?
      │  └─ Promote to first-class adapter + contract tests
      └─ Only needed for tests?
         └─ Move to test files + dependency injection
```

## Forbidden moves

These moves evade detection without fixing the violation. They are **never
acceptable**, regardless of whether the scan passes after the change:

1. **Rename identifiers** to avoid keyword detection
   (`mock_client` → `secondary_client`, `fake_repo` → `alt_repo`)

2. **Syntax-equivalent rewrites** that preserve the unauthorized decision
   (`.get(k, d)` → `d[k] if k in d else d`)

3. **Adding a new implicit default** where none existed before

4. **Keeping test-only dependencies in the runtime graph**
   (importing `unittest.mock` in production, even under a different alias)

5. **Inlining the default** rather than making ownership explicit
   (`value = data["key"]` with a bare `except KeyError: value = "default"`)

## Proof requirement

Every fix must add at least one machine-checkable proof that the violation
is resolved at the design level, not just the syntax level:

| Proof type | Example |
|-|-|
| **Type exhaustiveness** | `assert_never()` for Python unions, `never` for TypeScript |
| **Runtime validation** | Pydantic `model_validate()`, Zod `.parse()` at boundary |
| **Contract test** | Shared test suite that all implementations of a port must pass |
| **Architecture rule** | Import restriction, layer dependency check |

A fix without proof is incomplete. If you cannot add proof, flag the
violation for human review rather than guessing.

## Process

When you encounter a code-guardrails violation:

1. **Do NOT fix inline in the main conversation.** Delegate to a subagent
   via `/fix` or the Agent tool to preserve main context.

2. **Read the full file** to understand why the violation exists.

3. **Identify the owner layer** using the decision tree above.

4. **Choose exactly one legal remedy.** Do not combine remedies.

5. **Apply the fix and add proof.** Type annotation, validation, contract
   test, or architecture rule.

6. **Verify with the scan command.** Run the engine on the fixed file.

7. **Report what you changed:** which owner layer, which remedy, what proof.

## Language-specific guidance

- **Python**: See [references/remedies-python.md](references/remedies-python.md)
  for Pydantic, mypy --strict, assert_never, Hypothesis patterns.

- **TypeScript**: See [references/remedies-typescript.md](references/remedies-typescript.md)
  for strict/strictNullChecks, Zod, satisfies, never patterns.
