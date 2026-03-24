---
name: scan
description: Scan the project for policy violations (test doubles and unapproved fallbacks)
---

Run the code-guardrails policy scanner on the **user's current project** (not the plugin directory).

Execute this command:
```bash
python3 "${CLAUDE_PLUGIN_ROOT}/check_policy.py" --config-dir "${CLAUDE_PLUGIN_ROOT}" "$(pwd)" --format human
```

`--config-dir` points to the plugin directory (where sgconfig.yml and rules/ live).
The positional argument is the user's working directory (the project to scan).

Show the full output to the user. If violations are found:
1. List each violation with file, line, rule, and code snippet
2. Offer to fix them (remove test doubles, replace fallbacks with proper error handling)
3. For intentional fallbacks, offer to add `# policy-approved: REQ-xxx <reason>` comments
