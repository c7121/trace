# Testing Strategy

BDD-driven testing for agent-based development. Feature files serve as both spec and acceptance criteria.

## Philosophy

1. **Feature files are spec** — agents read them to understand what to build
2. **Tests verify acceptance** — each scenario maps to a testable outcome
3. **Move fast** — start with mocks, add integration tests incrementally

---

## Structure

```
/tests
  /features                    # Gherkin specs (readable by agents)
    /phase0_infra
      vpc.feature
      rds.feature
      sqs.feature
    /phase1_orchestration
      dispatcher.feature
      lambda_sources.feature
      worker.feature
    /phase2_ingestion
      block_follower.feature
      cryo_ingest.feature
      reorg_handling.feature
    /phase3_query
      query_service.feature
    /phase7_alerting
      alert_evaluate.feature
      alert_deliver.feature
      alert_dedup.feature
  /steps                       # Step definitions (Rust)
    mod.rs
    common.rs
    block_follower.rs
    query.rs
  /fixtures                    # Test data
    blocks.json
    reorg_scenario.json
  cucumber.rs                  # Test harness entry point
```

---

## Test Layers

| Layer | Tool | Purpose | Speed |
|-------|------|---------|-------|
| **Unit** | `cargo test` | Logic, no I/O | Fast |
| **Integration** | testcontainers-rs | Real Postgres | ~2s setup |
| **RPC Mock** | wiremock | Simulate chain responses | Fast |
| **E2E** | Staging env | Full system | Manual/CI |

### Unit Tests

Mock DB and external dependencies, test pure logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_reorg_detection() {
        let parent_hash = "0xabc";
        let expected_parent = "0xdef";
        assert!(detect_reorg(parent_hash, expected_parent));
    }
}
```

### Integration Tests (testcontainers-rs)

Real Postgres, spun up per test:

```rust
use testcontainers::{clients::Cli, images::postgres::Postgres};
use sqlx::PgPool;

#[tokio::test]
async fn test_block_insertion() {
    let docker = Cli::default();
    let pg_container = docker.run(Postgres::default());
    let port = pg_container.get_host_port_ipv4(5432);
    let conn_str = format!("postgres://postgres:postgres@localhost:{}/postgres", port);
    
    let pool = PgPool::connect(&conn_str).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    
    // Test with real Postgres
    insert_block(&pool, &test_block()).await.unwrap();
    let block = get_block(&pool, 1).await.unwrap();
    assert_eq!(block.number, 1);
}
```

### RPC Mocking (wiremock)

Simulate RPC responses including reorgs:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_rpc_block_fetch() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "number": "0x3e8",
                "hash": "0xabc...",
                "parentHash": "0xdef..."
            }
        })))
        .mount(&mock_server)
        .await;
    
    let client = RpcClient::new(&mock_server.uri());
    let block = client.get_block(1000).await.unwrap();
    assert_eq!(block.number, 1000);
}
```

---

## BDD with cucumber-rs

### Setup

```toml
# Cargo.toml
[dev-dependencies]
cucumber = "0.20"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
testcontainers = "0.15"
wiremock = "0.5"
```

### Feature File Example

```gherkin
# tests/features/phase2_ingestion/block_follower.feature

Feature: Block Follower
  Real-time ingestion of blocks to hot storage

  Background:
    Given a clean hot_blocks table

  Scenario: Ingest new block
    Given the chain tip is at block 1000
    When block 1001 is produced
    Then hot_blocks should contain block 1001
    And block 1001 should have valid structure

  Scenario: Emit threshold event
    Given threshold_blocks is set to 100
    And hot_blocks contains 99 blocks
    When block 100 is ingested
    Then a threshold event should be emitted with start=1 end=100

  Scenario: Detect and handle reorg
    Given hot_blocks contains blocks 1-1000
    And block 1000 has hash "0xorphan"
    When new block 1000 arrives with parent "0xfork" at block 995
    Then blocks 996-1000 with old hashes should be deleted
    And blocks 996-1000 with new hashes should be inserted
    And data_invalidations should contain scope="row_range" for blocks 996-1000
```

### Step Definitions

```rust
// tests/steps/block_follower.rs

use cucumber::{given, when, then, World};
use sqlx::PgPool;

#[derive(Debug, Default, World)]
pub struct BlockFollowerWorld {
    pub pool: Option<PgPool>,
    pub rpc_mock: Option<MockServer>,
    pub last_event: Option<ThresholdEvent>,
}

#[given("a clean hot_blocks table")]
async fn clean_table(world: &mut BlockFollowerWorld) {
    let pool = setup_test_db().await;
    sqlx::query("TRUNCATE hot_blocks").execute(&pool).await.unwrap();
    world.pool = Some(pool);
}

#[given(regex = r"hot_blocks contains blocks (\d+)-(\d+)")]
async fn seed_blocks(world: &mut BlockFollowerWorld, start: u64, end: u64) {
    let pool = world.pool.as_ref().unwrap();
    for i in start..=end {
        insert_test_block(pool, i).await;
    }
}

#[when(regex = r"block (\d+) is produced")]
async fn produce_block(world: &mut BlockFollowerWorld, num: u64) {
    // Trigger block_follower to process mock RPC response
}

#[then(regex = r"hot_blocks should contain block (\d+)")]
async fn verify_block(world: &mut BlockFollowerWorld, num: u64) {
    let pool = world.pool.as_ref().unwrap();
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM hot_blocks WHERE block_number = $1)"
    )
    .bind(num as i64)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(exists, "Block {} not found in hot_blocks", num);
}

#[then("a threshold event should be emitted with start=1 end=100")]
async fn verify_threshold_event(world: &mut BlockFollowerWorld) {
    let event = world.last_event.as_ref().expect("No event emitted");
    assert_eq!(event.start_block, 1);
    assert_eq!(event.end_block, 100);
}
```

### Test Harness

```rust
// tests/cucumber.rs

mod steps;

use cucumber::World;
use steps::block_follower::BlockFollowerWorld;

#[tokio::main]
async fn main() {
    BlockFollowerWorld::cucumber()
        .run("tests/features/")
        .await;
}
```

### Run

```bash
# Run all feature tests
cargo test --test cucumber

# Run specific feature
cargo test --test cucumber -- --name "Block Follower"
```

---

## Agent Handoff Format

When assigning work to an agent:

```markdown
## Task: Implement reorg handling in block_follower

### Spec
- `/docs/architecture/operators/block_follower.md` — full operator spec
- `/docs/architecture/data_versioning.md` — invalidation model

### Acceptance Criteria
Implement scenarios from `/tests/features/phase2_ingestion/block_follower.feature`:
- "Detect and handle reorg"

### Verification
```bash
cargo test --test cucumber -- --name "Detect and handle reorg"
```

### Definition of Done
- [ ] Test passes
- [ ] No clippy warnings
- [ ] Code documented
```

---

## Reorg Test Fixtures

```json
// tests/fixtures/reorg_scenario.json
{
  "description": "5-block reorg at block 995",
  "initial_chain": [
    {"number": 995, "hash": "0xa995", "parent_hash": "0xa994"},
    {"number": 996, "hash": "0xa996", "parent_hash": "0xa995"},
    {"number": 997, "hash": "0xa997", "parent_hash": "0xa996"},
    {"number": 998, "hash": "0xa998", "parent_hash": "0xa997"},
    {"number": 999, "hash": "0xa999", "parent_hash": "0xa998"},
    {"number": 1000, "hash": "0xa1000", "parent_hash": "0xa999"}
  ],
  "canonical_chain": [
    {"number": 996, "hash": "0xb996", "parent_hash": "0xa995"},
    {"number": 997, "hash": "0xb997", "parent_hash": "0xb996"},
    {"number": 998, "hash": "0xb998", "parent_hash": "0xb997"},
    {"number": 999, "hash": "0xb999", "parent_hash": "0xb998"},
    {"number": 1000, "hash": "0xb1000", "parent_hash": "0xb999"}
  ],
  "fork_block": 995,
  "orphaned_blocks": [996, 997, 998, 999, 1000]
}
```

---

## CI Integration

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: postgres
        ports:
          - 5432:5432
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run unit tests
        run: cargo test --lib
      - name: Run feature tests
        run: cargo test --test cucumber
```

---

## Summary

| Phase | Testing Approach |
|-------|------------------|
| **Now** | Feature files as spec, unit tests with mocks |
| **Phase 1-2** | Add integration tests for orchestration + ingestion |
| **Phase 3+** | Wire up cucumber-rs for critical paths |
| **CI** | Unit + integration on every PR |
