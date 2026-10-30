//! HTTP client and code generation

mod client;
mod code_gen;
mod prepared;

pub use client::*;
pub use code_gen::*;
pub use prepared::*;

pub mod oauth;

mod codegen_targets;
