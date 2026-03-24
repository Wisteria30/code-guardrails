# Rule YAML Templates & ast-grep Patterns

## Naming Convention

```
{lang}-no-{policy_group}-{pattern-name}.yml
```

Examples: `py-no-fallback-bool-or.yml`, `ts-no-empty-catch.yml`, `py-no-test-double-identifier.yml`

## Python Rule Template

```yaml
id: py-no-{policy_group}-{pattern-name}
language: Python
severity: error
files:
  - '**/*.py'
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*_test.py'
  - '**/test_*.py'
  - '**/conftest.py'
  - '**/generated/**'
rule:
  pattern: <ast-grep pattern here>
message: '<what was detected>'
note: '<why it is a problem and how to fix it>'
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

## TypeScript Rule Template

```yaml
id: ts-no-{policy_group}-{pattern-name}
language: TypeScript
severity: error
files:
  - '**/*.ts'
  - '**/*.cts'
  - '**/*.mts'
ignores:
  - '**/test/**'
  - '**/tests/**'
  - '**/*.test.ts'
  - '**/*.spec.ts'
  - '**/__tests__/**'
  - '**/generated/**'
rule:
  pattern: <ast-grep pattern here>
message: '<what was detected>'
note: '<why it is a problem and how to fix it>'
metadata:
  policy_group: fallback
  approval_mode: adjacent_policy_comment
```

## ast-grep Pattern Reference

| Syntax | Meaning | Example |
|--------|---------|---------|
| `$VAR` | Single node (variable, expression) | `$VAR = $A or $B` |
| `$$$BODY` | Multiple nodes (list of statements) | `try: $$$BODY` |
| `any:` | OR — match any of the listed patterns | See below |
| `kind:` + `regex:` | AST node type + regex | Identifier matching |

## Pattern Templates by Type

**Assignment pattern** (common for fallback detection):
```yaml
rule:
  pattern: $VAR = $A or $B
```

**Function call pattern** (dangerous API calls):
```yaml
rule:
  pattern: logging.disable($LEVEL)
```

**Block structure pattern** (try/catch):
```yaml
rule:
  pattern: |
    try:
      $$$BODY
    except $ERR:
      pass
```

**Identifier pattern** (naming violations):
```yaml
rule:
  kind: identifier
  regex: '(?i)(mock|stub|fake)'
```

**Multiple variants** (use `any:` when one pattern isn't enough):
```yaml
rule:
  any:
    - pattern: $VAR = $A || $B
    - pattern: const $VAR = $A || $B
    - pattern: let $VAR = $A || $B
    - pattern: var $VAR = $A || $B
```

## Principles to Reduce False Positives

1. **Restrict to assignments**: Use `$VAR = $A || $B` instead of `$A || $B` (excludes conditions)
2. **Include declaration variants**: In TS, `const/let/var` are separate AST patterns — list them all
3. **Filter by argument count**: `$OBJ.get($KEY)` (1 arg = OK) vs `$OBJ.get($KEY, $DEFAULT)` (2 args = NG)
4. **Consider call context**: `router.get()` vs `dict.get()` look the same syntactically
