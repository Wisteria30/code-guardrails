# Python Remedies

Language-specific guidance for fixing code-guardrails violations in Python.

## Boundary parse with Pydantic

When raw data enters the system (JSON, dict, env vars), parse it at the
boundary into a validated model. Core code then operates on guaranteed types.

### Before (violation)
```python
def process_event(raw: dict):
    timeout = raw.get("timeout", 30)         # unauthorized default
    name = raw.get("name", "unknown")         # unauthorized default
    tool_use_id = raw.get("toolUseId", "tool") # unauthorized default
```

### After (boundary parse)
```python
from pydantic import BaseModel, ConfigDict

class EventPayload(BaseModel):
    model_config = ConfigDict(extra="forbid")
    timeout: int
    name: str
    toolUseId: str      # required — no default

def process_event(raw: dict):
    payload = EventPayload.model_validate(raw)  # raises ValidationError
    # From here, all fields are guaranteed present and typed
    timeout = payload.timeout
    name = payload.name
    tool_use_id = payload.toolUseId
```

If a field genuinely has a spec-approved default, declare it in the model:
```python
class EventPayload(BaseModel):
    model_config = ConfigDict(extra="forbid")
    timeout: int = 30   # default lives in the schema, not scattered in code
    name: str
    toolUseId: str
```

### Environment variables with BaseSettings
```python
from pydantic_settings import BaseSettings

class AppConfig(BaseSettings):
    database_url: str               # required — fails if missing
    log_level: str = "INFO"         # spec-approved default
    # policy-approved: REQ-45 default log level

# Parse once at startup, inject everywhere
config = AppConfig()
```

## Exhaustive handling with Optional and assert_never

When a value is genuinely optional, keep it as `Optional[T]` and handle
both cases. Never collapse to a default.

### Before (violation)
```python
user_name = data.get("name") or "anonymous"
```

### After (exhaustive handling)
```python
from typing import Optional

def get_display_name(name: Optional[str]) -> str:
    if name is not None:
        return name
    raise ValueError("Display name is required but was None")
```

### Union exhaustiveness with assert_never
```python
from typing import assert_never, Literal

Status = Literal["active", "inactive", "pending"]

def handle_status(status: Status) -> str:
    if status == "active":
        return "User is active"
    elif status == "inactive":
        return "User is inactive"
    elif status == "pending":
        return "User is pending"
    else:
        assert_never(status)  # mypy catches missing cases
```

## Typed exceptions instead of silent defaults

When a state should be unreachable, raise a typed exception.

### Before (violation)
```python
try:
    result = parse_input(data)
except Exception:
    pass  # swallowed error
```

### After (typed exception)
```python
class InputParseError(ValueError):
    """Raised when input data cannot be parsed."""

try:
    result = parse_input(data)
except ValueError as e:
    logger.error("Failed to parse input: %s", e)
    raise InputParseError(f"Invalid input: {e}") from e
```

## Test doubles: move to tests

### Before (violation in production code)
```python
from unittest.mock import Mock

mock_client = Mock(spec=HttpClient)
service = UserService(client=mock_client)
```

### After (dependency injection + test file)

Production code:
```python
# src/service.py
class UserService:
    def __init__(self, client: HttpClient):
        self._client = client
```

Test file:
```python
# tests/test_service.py
from unittest.mock import Mock

def test_user_service():
    mock_client = Mock(spec=HttpClient)
    service = UserService(client=mock_client)
```

## First-class adapter with contract tests

When an alternate implementation is needed in production (not just tests),
promote it to a first-class adapter with shared contract tests.

```python
# ports/repository.py
from abc import ABC, abstractmethod

class UserRepository(ABC):
    @abstractmethod
    def save(self, user: User) -> None: ...
    @abstractmethod
    def get(self, user_id: str) -> Optional[User]: ...

# adapters/sql_repository.py
class SqlUserRepository(UserRepository):
    def save(self, user: User) -> None:
        self._session.add(user)
    def get(self, user_id: str) -> Optional[User]:
        return self._session.query(User).get(user_id)

# adapters/in_memory_repository.py
class InMemoryUserRepository(UserRepository):
    """First-class adapter for testing and local dev.
    Passes the same contract tests as SqlUserRepository."""
    def __init__(self):
        self._store: dict[str, User] = {}
    def save(self, user: User) -> None:
        self._store[user.id] = user
    def get(self, user_id: str) -> Optional[User]:
        return self._store.get(user_id)  # OK: Optional return, not a default

# tests/contract/test_user_repository.py
import pytest
from hypothesis.stateful import RuleBasedStateMachine, rule

class UserRepositoryContract(RuleBasedStateMachine):
    """Shared contract: every UserRepository implementation must pass this."""
    @rule(target=users, user=user_strategy())
    def save_user(self, user):
        self.repo.save(user)
        return user

    @rule(user=users)
    def get_saved_user(self, user):
        result = self.repo.get(user.id)
        assert result == user  # save then get returns same user

# Both implementations run the same contract
class TestSqlRepository(UserRepositoryContract):
    def __init__(self):
        super().__init__()
        self.repo = SqlUserRepository(test_session())

class TestInMemoryRepository(UserRepositoryContract):
    def __init__(self):
        super().__init__()
        self.repo = InMemoryUserRepository()
```

## mypy strict mode

Enable `mypy --strict` to catch missing type annotations, implicit `Any`,
and missing return types. This eliminates a large class of unauthorized
defaults that arise from weak typing.

```toml
# pyproject.toml
[tool.mypy]
strict = true
warn_return_any = true
disallow_untyped_defs = true
```
