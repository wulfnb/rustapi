//! Bounded HTTP subset of the v3 YAML directory format.
mod directory;
mod mapping;
use anyhow::{ensure, Result};
pub use directory::*;
use serde_json::Value;
pub fn parse_yaml(text: &str) -> Result<Value> {
    ensure!(text.len() <= 8 * 1024 * 1024, "YAML file exceeds 8 MiB");
    let options = serde_saphyr::options! {budget:serde_saphyr::budget!{max_depth:64,max_documents:1,max_aliases:0,max_anchors:0,max_events:200_000,max_nodes:100_000,max_total_scalar_bytes:8*1024*1024},reject_unsupported_tags:true,merge_keys:serde_saphyr::MergeKeyPolicy::Error};
    serde_saphyr::from_str_with_options(text, options)
        .map_err(|_| anyhow::anyhow!("Invalid v3 YAML or parsing budget exceeded"))
}
pub fn import_document(value: &Value) -> Result<crate::import::ImportResult> {
    crate::import::json(&serde_json::to_vec(&mapping::to_21(value, true)?)?)
}
