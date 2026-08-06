use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseRequest {
    pub model: String,
    pub instructions: String,
    pub input: String,
    pub max_output_tokens: Option<u32>,
}

impl ResponseRequest {
    pub fn new(
        model: impl Into<String>,
        instructions: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            model: model.into(),
            instructions: instructions.into(),
            input: input.into(),
            max_output_tokens: None,
        }
    }

    pub fn with_max_output_tokens(mut self, value: u32) -> Self {
        self.max_output_tokens = Some(value);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseResult {
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    pub incomplete_reason: Option<String>,
    pub usage: Option<ResponseUsage>,
    pub output_text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseUsage {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamDelta {
    pub delta: String,
}

#[derive(Clone, Debug)]
pub struct RequestOptions {
    pub timeout: Duration,
    pub cancellation: Option<CancellationToken>,
}

impl RequestOptions {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            cancellation: None,
        }
    }

    pub fn with_cancellation(mut self, cancellation: CancellationToken) -> Self {
        self.cancellation = Some(cancellation);
        self
    }
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}
