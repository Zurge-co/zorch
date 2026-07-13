//! Proxy-only provider integrations for AI model APIs.
//!
//! This crate no longer contains a high-level [`Provider`] trait.  Requests are
//! forwarded verbatim through [`ProxyProvider`](proxy::ProxyProvider), which
//! decrypts the provider API key, attaches the correct auth headers, and streams
//! the raw upstream response back to the client.
//!
//! Public exports:
//! - [`AuthHeaders`](auth_headers::AuthHeaders) and [`AuthType`](auth_headers::AuthType) — header builder
//! - [`ProviderHttpClient`](http_client::ProviderHttpClient) — retry-aware HTTP client
//! - [`ProxyProvider`](proxy::ProxyProvider) and [`ProviderApiKey`](auth_headers::ProviderApiKey)
//! - [`ProxyProviderRegistry`](registry::ProxyProviderRegistry) — runtime model/provider discovery
//! - [`BackendSelector`](selector::BackendSelector) — random/health-aware backend selection

mod auth_headers;
pub mod errors;
mod http_client;
pub mod model_resolver;
pub mod proxy;
pub mod registry;
pub mod selector;

pub use auth_headers::{AuthHeaders, AuthType, ProviderApiKey};
pub use errors::ProviderError;
pub use http_client::ProviderHttpClient;
pub use model_resolver::{ModelResolver, Target};
pub use proxy::{ProxiedResponse, ProxyProvider};
pub use registry::ProxyProviderRegistry;
pub use selector::{BackendSelector, RoutingStrategy};
