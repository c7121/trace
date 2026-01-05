use uuid::Uuid;

pub const TASK_CAPABILITY_HEADER: &str = "X-Trace-Task-Capability";

pub const CONTENT_TYPE_JSON: &str = "application/json";
pub const CONTENT_TYPE_JSONL: &str = "application/jsonl";

pub const DEFAULT_ALERT_DEFINITION_ID: Uuid = Uuid::from_bytes([
    // Stable fixture ID used by the harness when generating synthetic bundles/events.
    0x49, 0x0b, 0x8f, 0x3f, 0x1d, 0x41, 0x49, 0x6a, 0x91, 0x7b, 0x5b, 0x7e, 0xee, 0xb8, 0x5e, 0x07,
]);
