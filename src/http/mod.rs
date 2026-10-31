//! HTTP client and code generation

mod client;
mod code_gen;
mod prepared;

pub use client::*;
pub use code_gen::*;
pub use prepared::*;

pub mod oauth;

pub mod auth;
mod cookies;
pub use cookies::*;
mod cookie_bridge;
pub use cookie_bridge::*;
mod codegen_targets;
