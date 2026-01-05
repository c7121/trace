use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

fn default_work_payload() -> Value {
    Value::Object(Map::new())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UdfInvocationPayload {
    pub task_id: Uuid,
    pub attempt: i64,
    pub lease_token: Uuid,
    pub lease_expires_at: DateTime<Utc>,
    pub capability_token: String,

    #[serde(alias = "bundle_get_url")]
    pub bundle_url: String,

    #[serde(default = "default_work_payload")]
    pub work_payload: Value,
}
