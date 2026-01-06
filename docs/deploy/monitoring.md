# Monitoring

This page defines the **minimum viable monitoring** for Trace v1.

It is intentionally small. Do not spread numeric thresholds across multiple files; use:
- `docs/standards/operations.md` for default timings/limits and alert thresholds

## What to alert on

### Control plane health
- Dispatcher not running / unhealthy
- Postgres state connectivity errors
- Outbox stuck or failed rows (`outbox.status='Failed'` or old pending rows)
- Retry scheduler lag (tasks eligible to retry but not re-queued)

### Execution health
- Task queue depth and oldest message age
- Running tasks with near-expiry leases (heartbeat issues)
- High task failure rate (by job/operator)
- DLQ growth (task wake-ups and buffer queues)

### Data plane health
- Sink consumer failure rate
- Buffered dataset DLQ growth and replay backlog
- Query Service error rate / timeouts / export failures

### External delivery health
- Delivery terminal failures (exceeded max attempts)
- Per-destination failure rate spikes (webhooks, Slack, etc.)

## Logs

- Structured JSON logs to CloudWatch.
- Never log bearer tokens (user JWTs or task capability tokens).
- Use a separate audit stream if you need longer retention than debug logs (see `docs/standards/operations.md`).

## Related

- [infrastructure.md](infrastructure.md) - CloudWatch, ECS, and AWS layout
- [operations.md](../standards/operations.md) - defaults and runbooks
