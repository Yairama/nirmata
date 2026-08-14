pub mod capabilities;
pub mod contracts;

mod chat;
mod runtime;

pub use chat::{
    RequestOptions, ResponseJsonSchema, ResponseRequest, ResponseResult, ResponseUsage, StreamDelta,
};
pub use runtime::{
    AiError, AzureFoundryClient, CredentialPersistence, CredentialSource, ProviderCredentialStatus,
    ProviderCredentialStore,
};
pub(crate) use runtime::{
    AzureFoundryClientInner, ReqwestTransport, Transport, normalize_base_url,
};
#[cfg(test)]
pub(crate) use runtime::{TransportError, TransportRequest, TransportResponse};
pub use tokio_util::sync::CancellationToken;
