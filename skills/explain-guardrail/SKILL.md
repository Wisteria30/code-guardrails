---
name: explain-guardrail
description: >
  Explain what a code-guardrails violation means, why it matters, and how
  to fix it. Use when the user asks "what does this mean", "why is this
  blocked", "explain this violation", or wants to understand a guardrail
  rule or semantic class.
---

# Explain Guardrail

When asked to explain a code-guardrails violation, provide context from
the repair doctrine perspective — not just "this pattern is banned."

## How to explain

1. **Identify the semantic class** from the violation output:
   - `fallback_unowned_default` — An unauthorized default value. The code chose a fallback that the spec never authorized.
   - `fallback_unowned_handler` — Error swallowing. The code silences an error that should be logged, re-raised, or handled explicitly.
   - `boundary_parse_missing` — Raw data access outside the boundary. Environment variables, untyped dicts, or raw JSON should be parsed at the boundary layer.
   - `runtime_double_in_graph` — Test-only code in production. Mocks, stubs, fakes, or test framework imports in the runtime dependency graph.
   - `keyword_placeholder` — Suspicious keyword in a comment suggesting AI-introduced placeholder code.

2. **Explain the ownership problem**: The violation exists because a decision that belongs to a specific architectural layer is being made in the wrong place.

3. **Name the owner layer**: boundary, domain, application, infrastructure, composition root, or test.

4. **List the legal remedies** for this specific semantic class. Reference the repair doctrine skill for details:
   ${CLAUDE_PLUGIN_ROOT}/skills/repair-doctrine/SKILL.md

5. **Give a concrete example** of what the fixed code would look like, referencing the language-specific remedies:
   - Python: ${CLAUDE_PLUGIN_ROOT}/skills/repair-doctrine/references/remedies-python.md
   - TypeScript: ${CLAUDE_PLUGIN_ROOT}/skills/repair-doctrine/references/remedies-typescript.md

## Key framing

A fallback is not a coding style issue — it is an **unauthorized effect handler**
(a map δ: 1→A or h: E→A that the spec does not own).

A test double in production is not a naming issue — it is an **unverified runtime
substitute** (a model that may not satisfy the port's laws).

The fix is never "rewrite the line." The fix is "make the ownership explicit."
