# Worker-only contracts

Worker-only endpoints are callable only by trusted worker wrappers (ECS pollers). They must be protected by network policy (security groups allow only worker services) and a worker identity mechanism.

## Queue to worker (task wake-up)

Task queue message contains only `task_id` (wake-up). The worker then claims the task from the Dispatcher to obtain a lease and the full task payload. Duplicates are expected.

```json
{ "task_id": "uuid" }
```

## Authentication

Worker-only endpoints (`/internal/task-claim`, `/internal/task-fetch`) are called only by trusted worker wrappers and are protected by network policy plus a worker identity mechanism.

v1 worker identity is a shared worker token (rotatable secret) injected only into the wrapper container:

```
X-Trace-Worker-Token: <worker_token>
```

AWS note: ECS/Fargate tasks do not support per-container IAM roles. If you execute untrusted code in ECS, you must ensure it does not share AWS API permissions (SQS, queue ack, etc.) with the wrapper and does not receive the worker token.

## Task claim (`/internal/task-claim`)

Workers must claim a task before executing operator or UDF code. Claiming acquires a short-lived lease so only one worker may run the current attempt.

```
POST /internal/task-claim
X-Trace-Worker-Token: <worker_token>
```

Request:

```json
{
  "task_id": "uuid",
  "worker_id": "ecs:cluster/service/task"
}
```

Response (claimed):

```json
{
  "status": "Claimed",
  "attempt": 1,
  "lease_token": "uuid",
  "lease_expires_at": "2025-12-31T12:00:00Z",
  "capability_token": "jwt",
  "task": {
    "task_id": "uuid",
    "attempt": 1,
    "job": { "dag_name": "monad", "name": "block_follower" },
    "operator": "block_follower",
    "config": { "...": "..." },
    "inputs": [{ "...": "..." }]
  }
}
```

Response (not claimed):

```json
{ "status": "NotClaimed", "reason": "AlreadyRunning|Completed|Canceled|NotFound" }
```

If not claimed, the worker should not execute the task and should ack or delete the queue message.

## Task fetch (`/internal/task-fetch`)

Workers fetch task details by `task_id` (read-only from the worker perspective):

```
GET /internal/task-fetch?task_id=<uuid>
X-Trace-Worker-Token: <worker_token>
```

If the task is canceled (for example during rollback), the Dispatcher may return `status: "Canceled"`.
In that case the wrapper exits without running operator code and reports the cancellation via `POST /v1/task/complete` with `status: "Canceled"`.

## Related

- Task-scoped endpoints: [task_scoped_endpoints.md](task_scoped_endpoints.md)
- Task capability tokens: [task_capability_tokens.md](task_capability_tokens.md)
- Task lifecycle: [task_lifecycle.md](../task_lifecycle.md)

