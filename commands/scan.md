---
name: scan
description: Scan the project for policy violations (test doubles and unapproved fallbacks)
---

Run the code-guardrails policy scanner on the current project.

Execute this command:
```bash
cd "${CLAUDE_PLUGIN_ROOT}" && python3 check_policy.py "$(git rev-parse --show-toplevel 2>/dev/null || pwd)" --format human
```

Show the full output to the user. If violations are found:
1. List each violation with file, line, rule, and code snippet
2. Offer to fix them (remove test doubles, replace fallbacks with proper error handling)
3. For intentional fallbacks, offer to add `# policy-approved: REQ-xxx <reason>` comments
