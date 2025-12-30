# Product Requirements

## Purpose
A general-purpose data platform for blockchain research and operations: safe, reliable, and extensible.

## Users
- Analysts and researchers
- DeFi teams
- Security professionals

## User stories
As an analyst or researcher, I can:
- **Curate** onchain data:
    - select, filter, and organize datasets from blockchain networks.
- **Combine** onchain data with offchain feeds:
    - enrich blockchain data with external sources.
- **Enrich** data:
    - add labels, annotations, and computed fields (both real-time and retroactive).
- **Alert** on data:
    - define conditions and receive notifications on historical and live data.
- **Analyze** data:
    - run summaries, aggregations, and models across the dataset.
- **Access both historical and real-time data**:
    - seamless queries across full history and chain tip.

## Goals
- **Safe**: least privilege access; secrets managed securely; full audit trail.
- **Reliable**: no silent data loss; system recovers gracefully from failures.
- **Extensible**: variety of data in (onchain, offchain, batch, stream, push, pull); variety of operations out (query, enrich, alert, model).

## Non-goals
- Ultra-low-latency trading use cases
- On-prem deployment
- Multi-tenancy in v1 (single-tenant deployment; schema supports future multi-tenant expansion)

## Dependencies/assumptions
- Cloud: AWS (initial target; design should not preclude portability).
- Chain: start with Monad (EVM-compatible); architecture supports adding chains later.
- IaC: only path to provision infrastructure; no manual changes.

---

## Non-Functional Requirements

See [standards/nfr.md](../standards/nfr.md) for detailed targets.
