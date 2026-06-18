//! Proxy-only provider integrations for AI model APIs.
//!
//! This crate no longer contains a high-level [`Provider`] trait.  Requests are
//! forwarded verbatim through [`ProxyProvider`](proxy::ProxyProvider), which
//! decrypts the provider API key, attaches the correct auth headers, and streams
//! the raw upstream response back to the client.
//!
//! Public exports:
//! - [`AuthHeaders`](auth_headers::AuthHeaders) — header builder
//! - [`ProviderHttpClient`](http_client::ProviderHttpClient) — retry-aware HTTP client
//! - [`ProxyProvider`](proxy::ProxyProvider) and [`Protocol`](proxy::Protocol)
//! - [`ProxyProviderRegistry`](registry::ProxyProviderRegistry) — runtime model/provider discovery

mod auth_headers;
pub mod errors;
mod http_client;
pub mod proxy;
pub mod registry;

pub use errors::ProviderError;
pub use http_client::ProviderHttpClient;
pub use proxy::{Protocol, ProxyProvider};
pub use registry::ProxyProviderRegistry;
