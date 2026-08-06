use crate::{
    AiError, AzureFoundryClientInner, ChatCompletionRequest, ChatCompletionUsage, ChatMessage,
    RequestOptions, ReqwestTransport, Transport,
    contracts::{
        AdvisoryResponse, CritiqueReport, StructuredOutputError, parse_advisory_response,
        parse_change_set_draft, parse_critique_report,
    },
};
use nirmata_core::change_set::ChangeSetDraft;
use serde::Serialize;
use std::{error::Error, fmt};

pub const QUERY_PROMPT_VERSION: &str = "query_v1";
pub const PROPOSE_PROMPT_VERSION: &str = "propose_v1";
pub const CRITIC_PROMPT_VERSION: &str = "critic_v1";

const QUERY_SYSTEM_PROMPT: &str = concat!(
    "Modo query de Nirmata. ",
    "Responde solo con JSON advisory_response. ",
    "No emitas operaciones, mutaciones, ChangeSetDraft ni texto fuera del contrato. ",
    "Cada hecho, inferencia o perspectiva debe citar fuentes del contexto. ",
    "Si falta evidencia, responde con no_evidence o unspecified y no inventes citas ni content references."
);

const PROPOSE_SYSTEM_PROMPT: &str = concat!(
    "Modo propose de Nirmata. ",
    "Responde solo con un objeto JSON change_set_draft. ",
    "Usa la revision base y las fuentes del contexto entregado. ",
    "No respondas con otros contratos ni texto libre."
);

const CRITIC_SYSTEM_PROMPT: &str = concat!(
    "Modo critic de Nirmata. ",
    "Responde solo con JSON critique_report. ",
    "Evalua el draft recibido contra el snapshot y las fuentes. ",
    "No edites el draft ni propongas un replacement draft."
);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationStatus {
    Completed,
    Truncated,
    ContentFiltered,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvocationMetadata {
    pub model: String,
    pub prompt_version: String,
    pub context_object_ids: Vec<String>,
    pub status: InvocationStatus,
    pub usage: Option<ChatCompletionUsage>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityInvocation<T> {
    pub output: T,
    pub metadata: InvocationMetadata,
}

#[derive(Debug)]
pub enum CapabilityError {
    Ai(AiError),
    Serialization(String),
    StructuredOutput(StructuredOutputError),
}

impl fmt::Display for CapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ai(error) => error.fmt(formatter),
            Self::Serialization(message) => write!(formatter, "{message}"),
            Self::StructuredOutput(error) => error.fmt(formatter),
        }
    }
}

impl Error for CapabilityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Ai(error) => Some(error),
            Self::StructuredOutput(error) => Some(error),
            Self::Serialization(_) => None,
        }
    }
}

pub struct AzureFoundryCapabilityClient {
    inner: CapabilityClientInner<ReqwestTransport>,
}

impl AzureFoundryCapabilityClient {
    pub fn new(
        base_url: &str,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, AiError> {
        Ok(Self {
            inner: CapabilityClientInner::with_client(
                AzureFoundryClientInner::new(
                    crate::normalize_base_url(base_url)?,
                    ReqwestTransport::default(),
                ),
                api_key,
                model,
            ),
        })
    }

    pub async fn query<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>
    where
        P: Serialize,
    {
        self.inner.query(payload, context_object_ids, options).await
    }

    pub async fn query_streaming<P, F>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
        on_delta: F,
    ) -> Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>
    where
        P: Serialize,
        F: FnMut(crate::StreamDelta) + Send,
    {
        self.inner
            .query_streaming(payload, context_object_ids, options, on_delta)
            .await
    }

    pub async fn propose<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>
    where
        P: Serialize,
    {
        self.inner
            .propose(payload, context_object_ids, options)
            .await
    }

    pub async fn critic<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<CritiqueReport>, CapabilityError>
    where
        P: Serialize,
    {
        self.inner
            .critic(payload, context_object_ids, options)
            .await
    }
}

struct CapabilityClientInner<T> {
    client: AzureFoundryClientInner<T>,
    api_key: String,
    model: String,
}

impl<T> CapabilityClientInner<T>
where
    T: Transport,
{
    fn with_client(
        client: AzureFoundryClientInner<T>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            client,
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    async fn query<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>
    where
        P: Serialize,
    {
        self.invoke(
            payload,
            context_object_ids,
            QUERY_SYSTEM_PROMPT,
            QUERY_PROMPT_VERSION,
            2_048,
            parse_advisory_response,
            options,
        )
        .await
    }

    async fn query_streaming<P, F>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
        on_delta: F,
    ) -> Result<CapabilityInvocation<AdvisoryResponse>, CapabilityError>
    where
        P: Serialize,
        F: FnMut(crate::StreamDelta) + Send,
    {
        self.invoke_streaming(
            payload,
            context_object_ids,
            QUERY_SYSTEM_PROMPT,
            QUERY_PROMPT_VERSION,
            2_048,
            parse_advisory_response,
            options,
            on_delta,
        )
        .await
    }

    async fn propose<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<ChangeSetDraft>, CapabilityError>
    where
        P: Serialize,
    {
        self.invoke(
            payload,
            context_object_ids,
            PROPOSE_SYSTEM_PROMPT,
            PROPOSE_PROMPT_VERSION,
            4_096,
            |raw| parse_change_set_draft(raw).map(|draft| draft.into_inner()),
            options,
        )
        .await
    }

    async fn critic<P>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<CritiqueReport>, CapabilityError>
    where
        P: Serialize,
    {
        self.invoke(
            payload,
            context_object_ids,
            CRITIC_SYSTEM_PROMPT,
            CRITIC_PROMPT_VERSION,
            4_096,
            parse_critique_report,
            options,
        )
        .await
    }

    async fn invoke<P, O>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        system_prompt: &'static str,
        prompt_version: &'static str,
        max_output_tokens: u32,
        parse_output: impl FnOnce(&str) -> Result<O, StructuredOutputError>,
        options: RequestOptions,
    ) -> Result<CapabilityInvocation<O>, CapabilityError>
    where
        P: Serialize,
    {
        let user_payload = serde_json::to_string(payload)
            .map_err(|error| CapabilityError::Serialization(error.to_string()))?;
        let response = self
            .client
            .complete_chat(
                &self.api_key,
                ChatCompletionRequest::new(
                    self.model.clone(),
                    vec![
                        ChatMessage::system(system_prompt),
                        ChatMessage::user(user_payload),
                    ],
                )
                .with_temperature(0.0)
                .with_max_output_tokens(max_output_tokens),
                options,
            )
            .await
            .map_err(CapabilityError::Ai)?;
        let metadata = InvocationMetadata {
            model: response.model.clone().unwrap_or_else(|| self.model.clone()),
            prompt_version: prompt_version.to_owned(),
            context_object_ids,
            status: status_from_finish_reason(response.finish_reason.as_deref()),
            usage: response.usage.clone(),
        };
        let output =
            parse_output(&response.output_text).map_err(CapabilityError::StructuredOutput)?;
        Ok(CapabilityInvocation { output, metadata })
    }

    async fn invoke_streaming<P, O, F>(
        &self,
        payload: &P,
        context_object_ids: Vec<String>,
        system_prompt: &'static str,
        prompt_version: &'static str,
        max_output_tokens: u32,
        parse_output: impl FnOnce(&str) -> Result<O, StructuredOutputError>,
        options: RequestOptions,
        on_delta: F,
    ) -> Result<CapabilityInvocation<O>, CapabilityError>
    where
        P: Serialize,
        F: FnMut(crate::StreamDelta) + Send,
    {
        let user_payload = serde_json::to_string(payload)
            .map_err(|error| CapabilityError::Serialization(error.to_string()))?;
        let response = self
            .client
            .stream_chat(
                &self.api_key,
                ChatCompletionRequest::new(
                    self.model.clone(),
                    vec![
                        ChatMessage::system(system_prompt),
                        ChatMessage::user(user_payload),
                    ],
                )
                .with_temperature(0.0)
                .with_max_output_tokens(max_output_tokens),
                options,
                on_delta,
            )
            .await
            .map_err(CapabilityError::Ai)?;
        let metadata = InvocationMetadata {
            model: response.model.clone().unwrap_or_else(|| self.model.clone()),
            prompt_version: prompt_version.to_owned(),
            context_object_ids,
            status: status_from_finish_reason(response.finish_reason.as_deref()),
            usage: response.usage.clone(),
        };
        let output =
            parse_output(&response.output_text).map_err(CapabilityError::StructuredOutput)?;
        Ok(CapabilityInvocation { output, metadata })
    }
}

fn status_from_finish_reason(reason: Option<&str>) -> InvocationStatus {
    match reason {
        Some("stop") => InvocationStatus::Completed,
        Some("length") => InvocationStatus::Truncated,
        Some("content_filter") => InvocationStatus::ContentFiltered,
        _ => InvocationStatus::Unknown,
    }
}

#[cfg(test)]
#[path = r"..\tests\capabilities\mod.rs"]
mod tests;
