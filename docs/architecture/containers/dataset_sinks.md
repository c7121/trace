# Dataset Sinks

Dataset Sinks are trusted platform workers that consume buffered dataset records from SQS and write them into Postgres data.

This pattern is used when:

- many producers need to write to the same logical dataset, and/or
- direct Postgres writes from producers are undesirable (credential isolation), and/or
- we want queue-based backpressure and retry.

See ADR 0006 for the full design.

## Responsibilities

- Consume messages from dataset buffer queues (SQS).
- Enforce idempotency using stable `dedupe_key` and unique constraints.
- Write to the target Postgres dataset table(s).
- Emit sink health metrics (queue age, lag, error rate).

## Component View

```mermaid
flowchart LR
    buffer[[SQS dataset buffers]]:::infra
    sink{{Sink worker}}:::component
    writer["Dedupe and upsert"]:::component
    pg[(Postgres data)]:::database
    obs[Observability]:::ext

    buffer -->|messages| sink
    sink -->|apply dedupe_key| writer
    writer -->|insert or upsert| pg
    sink -->|metrics| obs

    classDef component fill:#d6ffe7,stroke:#1f9a6f,color:#000;
    classDef database fill:#fff6d6,stroke:#c58b00,color:#000;
    classDef infra fill:#e8e8ff,stroke:#6666aa,color:#000;
    classDef ext fill:#eee,stroke:#666,color:#000;
```
