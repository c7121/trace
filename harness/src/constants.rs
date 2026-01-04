use uuid::Uuid;

pub const TASK_CAPABILITY_HEADER: &str = "X-Trace-Task-Capability";

pub const CONTENT_TYPE_JSON: &str = "application/json";
pub const CONTENT_TYPE_JSONL: &str = "application/jsonl";

pub const DEFAULT_ALERT_DEFINITION_ID: Uuid = Uuid::from_bytes([
    // Stable fixture ID used by the harness when generating synthetic bundles/events.
    0x49, 0x0b, 0x8f, 0x3f, 0x1d, 0x41, 0x49, 0x6a, 0x91, 0x7b, 0x5b, 0x7e, 0xee, 0xb8, 0x5e, 0x07,
]);

pub const OUTBOX_NAMESPACE: Uuid = Uuid::from_bytes([
    // UUIDv5 namespace for deterministic outbox message IDs (task fencing/idempotency).
    0x6c, 0x07, 0x30, 0x87, 0x5b, 0x7c, 0x4c, 0x55, 0xb0, 0x7a, 0x1e, 0x2c, 0x7a, 0x01, 0x5a, 0xe2,
]);
