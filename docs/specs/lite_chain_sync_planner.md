# Lite chain sync planner (ms/13)

Risk: Medium
Public surface: Postgres state tables `state.chain_sync_cursor`, `state.chain_sync_scheduled_ranges`; dispatcher CLI `plan-chain-sync`

Summary: Add an idempotent planner that schedules bounded `cryo_ingest` range tasks from genesis to tip and persists progress.

Plan:
- Add cursor + scheduled-ranges tables; schedule ranges transactionally with outbox wakeups.
- Mark scheduled ranges completed on successful task completion.

Acceptance:
- Running planner twice schedules each range once and cursor advances monotonically.
- Harness test proves plan→cryo_worker→dataset_versions registered once per range and scheduled ranges complete.

Reduction:
- No RPC tip lookup; require explicit `to_block` for v1 tests.
