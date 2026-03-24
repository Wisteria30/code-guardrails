---
name: guardrails
description: Understand the code-guardrails policy model for writing compliant code
---

# Code Guardrails Policy

This project enforces two policies on production code:

## 1. No Test Doubles in Production
- mock, stub, fake identifiers are banned outside test files
- `unittest.mock` imports are banned outside test files
- No exceptions. Use MagicMock in your unit tests only.

## 2. No Unapproved Fallbacks
- Default values in `.get()`, `getattr()`, `next()`, `os.getenv()` are flagged
- `or` fallbacks in assignments (`x = a or b`) are flagged
- `??`, `||`, `??=`, `||=` in TypeScript are flagged
- `except: pass`, `contextlib.suppress`, empty catch blocks are flagged
- Promise `.catch(() => default)` is flagged

### Approval Model
To approve an intentional fallback, add a comment on the same line or within 2 lines above:

```python
# policy-approved: REQ-123 explicit locale default
lang = payload.get("lang", "ja-JP")
```

The comment must contain `policy-approved:` followed by a `REQ-`, `ADR-`, or `SPEC-` prefix and an identifier.

## When Writing Code
- Never introduce mock/stub/fake objects in production code
- Never add fallback defaults without explicit requirement approval
- If a fallback is needed, discuss with the user and get a REQ/ADR/SPEC reference first
