# Trace

A general-purpose data platform for blockchain research and operations.

## What is Trace?

Trace lets analysts and researchers curate, combine, enrich, alert on, and analyze onchain and offchain data, both historical and real-time.

User stories:
- Curate onchain data - select, filter, and organize datasets from blockchain networks
- Combine onchain data with offchain feeds - enrich blockchain data with external sources
- Enrich data - add labels, annotations, and computed fields, real-time and retroactive
- Alert on data - define conditions and receive notifications on historical and live data
- Analyze data - run summaries, aggregations, and models across the dataset
- Access both historical and real-time data - seamless queries across full history and chain tip

Goals:
- Safe - least privilege access, secrets managed securely, full audit trail
- Reliable - no silent data loss, system recovers gracefully from failures
- Extensible - variety of data in and variety of operations out

Non-goals:
- Ultra-low-latency trading
- On-prem deployment
- Multi-tenancy in v1

Assumptions:
- AWS, with a portable design
- Monad-first (EVM-compatible, multi-chain ready)
- IaC-only provisioning

## Documentation

- Docs portal: [docs/readme.md](docs/readme.md)
- Implementers: [docs/architecture/README.md](docs/architecture/README.md)
- Trace Lite harness: [harness/README.md](harness/README.md)

## Status

Design phase. See [backlog.md](docs/plan/backlog.md) for implementation roadmap.
