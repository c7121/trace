use async_trait::async_trait;

use crate::{udf::UdfInvocationPayload, Result};

/// Invoke an untrusted UDF runtime using a task-scoped payload.
///
/// The invoked runtime is responsible for calling back into the Dispatcher to publish outputs
/// (buffer-publish) and to complete the task.
#[async_trait]
pub trait RuntimeInvoker: Send + Sync {
    async fn invoke(&self, invocation: &UdfInvocationPayload) -> Result<()>;
}
