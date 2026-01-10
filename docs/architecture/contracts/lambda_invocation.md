# Dispatcher to Lambda invocation contract

For jobs with `runtime: lambda`, the Dispatcher invokes the Lambda directly (no task queue).

In AWS, `runtime: lambda` should refer to a platform-managed runner Lambda per environment, not user-deployed Lambdas.

- The runner treats the bundle as untrusted code.
- The runner execution role should be near-zero (no broad S3/SQS/Secrets Manager access).
- The Dispatcher should supply an object-scoped pre-signed S3 GET URL for the bundle so the runner does not need S3 IAM permissions.

Treat Lambda as untrusted by default: do not rely on hidden internal credentials.
There is no wrapper boundary in Lambda; do not rely on hidden shared secrets.

For task execution, the Dispatcher includes a per-attempt task capability token in the invocation payload. The Lambda uses that token to:
- read data via Query Service (`/v1/task/query`),
- obtain scoped object-store credentials (`/v1/task/credentials`), and
- report fenced heartbeat, completion, and events to the Dispatcher (`/v1/task/*`).

Invocation payload includes the full task payload (same shape as `/internal/task-fetch`) so the Lambda does not need to fetch task details before executing:

```json
{
  "task_id": "uuid",
  "attempt": 1,
  "lease_token": "uuid",
  "lease_expires_at": "2025-12-31T12:00:00Z",
  "capability_token": "jwt",
  "bundle_url": "https://s3.../udf/{bundle}.zip?X-Amz-...",
  "job": { "dag_name": "monad", "name": "block_follower" },
  "operator": "block_follower",
  "config": { "...": "..." },
  "inputs": [{ "...": "..." }]
}
```

Exact payload fields are still evolving; the invariant is that Lambda has everything it needs to run the operator and report fenced completion without Postgres state credentials.

The Dispatcher acquires a lease before invoking (transitions the task to Running) and includes `(attempt, lease_token)` in the payload.

The Lambda follows the same contract as workers: heartbeat (optional) and report completion, failure, and events via the task-scoped Dispatcher endpoints. Task lifecycle (timeouts, retries) is defined in [task_lifecycle.md](../task_lifecycle.md).

Lambda built-in retries should be disabled; the Dispatcher owns retries and attempts uniformly across runtimes.

Small Lambda operators can be implemented in TypeScript or JavaScript, Rust, or Python.

## Related

- Task capability token: [task_capability_tokens.md](task_capability_tokens.md)
- Task-scoped endpoints: [task_scoped_endpoints.md](task_scoped_endpoints.md)
- Worker-only endpoints: [worker_only_endpoints.md](worker_only_endpoints.md)
