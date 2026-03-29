# TypeScript Remedies

Language-specific guidance for fixing code-guardrails violations in TypeScript.

## Boundary parse with Zod

When raw data enters the system (API response, request body, env vars),
parse it at the boundary into a validated type. Core code then operates
on guaranteed types.

### Before (violation)
```typescript
function processEvent(raw: Record<string, unknown>) {
  const timeout = raw.timeout ?? 30;           // unauthorized default
  const name = raw.name || "unknown";          // unauthorized default
  const toolUseId = raw.toolUseId ?? "tool";   // unauthorized default
}
```

### After (boundary parse with Zod)
```typescript
import { z } from "zod";

const EventPayload = z.object({
  timeout: z.number(),
  name: z.string(),
  toolUseId: z.string(),
}).strict();  // rejects extra fields

type EventPayload = z.infer<typeof EventPayload>;

function processEvent(raw: unknown) {
  const payload = EventPayload.parse(raw);  // throws ZodError
  // From here, all fields are guaranteed present and typed
  const { timeout, name, toolUseId } = payload;
}
```

If a field genuinely has a spec-approved default, declare it in the schema:
```typescript
const EventPayload = z.object({
  timeout: z.number().default(30),  // default lives in schema
  name: z.string(),
  toolUseId: z.string(),
}).strict();
```

### Environment variables with Zod
```typescript
const EnvSchema = z.object({
  DATABASE_URL: z.string().url(),          // required
  LOG_LEVEL: z.string().default("INFO"),   // spec-approved default
  PORT: z.coerce.number().default(3000),   // spec-approved default
});

// Parse once at startup, inject everywhere
const env = EnvSchema.parse(process.env);
```

## Exhaustive handling with strict and never

When a value is genuinely optional, keep it as `T | null` and handle
both cases. Enable `strictNullChecks` so the compiler enforces this.

### Before (violation)
```typescript
const userName = data.name ?? "anonymous";
```

### After (exhaustive handling)
```typescript
function getDisplayName(name: string | null): string {
  if (name !== null) {
    return name;
  }
  throw new Error("Display name is required but was null");
}
```

### Union exhaustiveness with never
```typescript
type Status = "active" | "inactive" | "pending";

function handleStatus(status: Status): string {
  switch (status) {
    case "active":
      return "User is active";
    case "inactive":
      return "User is inactive";
    case "pending":
      return "User is pending";
    default:
      // TypeScript catches missing cases at compile time
      const _exhaustive: never = status;
      throw new Error(`Unexpected status: ${_exhaustive}`);
  }
}
```

## Typed errors instead of silent defaults

When a state should be unreachable or an operation can fail, use typed
errors rather than swallowing exceptions.

### Before (violation)
```typescript
const result = await fetchData().catch(() => null);  // swallowed error

try {
  const parsed = JSON.parse(input);
} catch {
  // empty catch — error silently swallowed
}
```

### After (typed error)
```typescript
class FetchError extends Error {
  constructor(
    message: string,
    public readonly cause: unknown,
  ) {
    super(message);
    this.name = "FetchError";
  }
}

async function fetchDataSafe(): Promise<Data> {
  try {
    return await fetchData();
  } catch (error: unknown) {
    throw new FetchError("Failed to fetch data", error);
  }
}

// Or with Result type for expected failures
type Result<T, E = Error> = { ok: true; value: T } | { ok: false; error: E };

async function fetchDataResult(): Promise<Result<Data, FetchError>> {
  try {
    const data = await fetchData();
    return { ok: true, value: data };
  } catch (error: unknown) {
    return { ok: false, error: new FetchError("Fetch failed", error) };
  }
}
```

## Test doubles: move to tests

### Before (violation in production code)
```typescript
const fakeRepository = new FakeUserRepository();
const service = new UserService(fakeRepository);
```

### After (dependency injection + test file)

Production code:
```typescript
// src/service.ts
export class UserService {
  constructor(private readonly repository: UserRepository) {}
}
```

Test file:
```typescript
// src/__tests__/service.test.ts
class FakeUserRepository implements UserRepository {
  private store = new Map<string, User>();
  async save(user: User): Promise<void> {
    this.store.set(user.id, user);
  }
  async get(id: string): Promise<User | null> {
    return this.store.get(id) ?? null;  // OK: Optional return from a test double
  }
}

test("UserService creates user", async () => {
  const repo = new FakeUserRepository();
  const service = new UserService(repo);
  // ...
});
```

## First-class adapter with contract tests

When an alternate implementation is needed in production, promote it to a
first-class adapter with shared contract tests.

```typescript
// ports/repository.ts
export interface UserRepository {
  save(user: User): Promise<void>;
  get(id: string): Promise<User | null>;
}

// adapters/sql-repository.ts
export class SqlUserRepository implements UserRepository {
  constructor(private readonly db: Database) {}
  async save(user: User): Promise<void> {
    await this.db.insert("users", user);
  }
  async get(id: string): Promise<User | null> {
    return await this.db.findOne("users", { id });
  }
}

// adapters/in-memory-repository.ts
export class InMemoryUserRepository implements UserRepository {
  private store = new Map<string, User>();
  async save(user: User): Promise<void> {
    this.store.set(user.id, user);
  }
  async get(id: string): Promise<User | null> {
    return this.store.get(id) ?? null;  // OK: Optional return from a first-class adapter
  }
}

// tests/contract/user-repository.contract.ts
export function userRepositoryContract(
  createRepo: () => UserRepository,
) {
  describe("UserRepository contract", () => {
    let repo: UserRepository;
    beforeEach(() => { repo = createRepo(); });

    it("returns saved user by id", async () => {
      const user = createTestUser();
      await repo.save(user);
      const result = await repo.get(user.id);
      expect(result).toEqual(user);
    });

    it("returns null for unknown id", async () => {
      const result = await repo.get("nonexistent");
      expect(result).toBeNull();
    });
  });
}

// Both implementations run the same contract
// tests/sql-repository.test.ts
userRepositoryContract(() => new SqlUserRepository(testDb()));

// tests/in-memory-repository.test.ts
userRepositoryContract(() => new InMemoryUserRepository());
```

## TypeScript strict configuration

Enable strict mode and all strict-family flags to catch unauthorized
defaults at compile time:

```json
// tsconfig.json
{
  "compilerOptions": {
    "strict": true,
    "strictNullChecks": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitReturns": true
  }
}
```

`noUncheckedIndexedAccess` is particularly important: it makes `obj[key]`
return `T | undefined` instead of `T`, forcing explicit handling of missing
keys and eliminating a large class of unauthorized fallbacks.

## satisfies for type-safe configuration

Use `satisfies` to verify configuration objects match a type while preserving
literal types — catches unauthorized defaults at the type level:

```typescript
interface AppConfig {
  port: number;
  logLevel: "debug" | "info" | "warn" | "error";
}

// Type-checked at compile time, no runtime default needed
const config = {
  port: 3000,
  logLevel: "info",
} satisfies AppConfig;
```
