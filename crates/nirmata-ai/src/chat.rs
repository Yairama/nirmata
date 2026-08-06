use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: Option<serde_json::Number>,
    pub max_output_tokens: Option<u32>,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_output_tokens: None,
        }
    }

    pub fn with_temperature(mut self, value: f64) -> Self {
        self.temperature = serde_json::Number::from_f64(value);
        self
    }

    pub fn with_max_output_tokens(mut self, value: u32) -> Self {
        self.max_output_tokens = Some(value);
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionResponse {
    pub request_id: Option<String>,
    pub model: Option<String>,
    pub finish_reason: Option<String>,
    pub usage: Option<ChatCompletionUsage>,
    pub output_text: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
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
