//! Shared primitives used across the Zorch workspace.
//!
//! This crate contains type definitions, error types, configuration parsing,
//! cryptographic utilities, and procedural macros that eliminate boilerplate.
//! It sits at the bottom of the dependency graph and must not depend on any
//! other workspace crate.

#[macro_use]
pub mod macros;

pub mod config;
pub mod crypto;
pub mod errors;
pub mod types;

pub use config::AppConfig;
pub use crypto::SecretVault;
pub use errors::AppError;
pub use types::{ApiKeyTag, ApiKeyId, BackendId, ModelId, OrgId, ProviderApiKeyId, ProviderId, RequestId};
#[cfg(test)]
pub use types::{TokenCount, VirtualModelId};
pub use types::validate_tags;
