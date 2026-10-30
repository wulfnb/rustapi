//! Bounded, offline JSON Schema assertions. Error text never includes instance values.
use anyhow::{ensure, Result};
use serde_json::Value;
#[derive(Clone, Copy)]
enum Position {
    Schema,
    Map,
    Array,
    Data,
}
fn budget<'a>(
    root: &'a Value,
    node: &'a Value,
    depth: usize,
    left: &mut usize,
    stack: &mut Vec<usize>,
    position: Position,
) -> Result<()> {
    ensure!(depth <= 32 && *left > 0, "Schema complexity limit exceeded");
    *left -= 1;
    let pointer = node as *const Value as usize;
    ensure!(
        !stack.contains(&pointer),
        "Recursive schema references are unsupported"
    );
    stack.push(pointer);
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(position, Position::Schema)
                    && ["$ref", "$dynamicRef", "$recursiveRef"].contains(&key.as_str())
                {
                    let reference = value
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("Schema reference must be text"))?;
                    ensure!(
                        reference == "#" || reference.starts_with("#/"),
                        "Only local JSON Pointer schema references are supported"
                    );
                    let target = root
                        .pointer(reference.strip_prefix('#').unwrap())
                        .ok_or_else(|| anyhow::anyhow!("Missing local schema reference"))?;
                    budget(root, target, depth + 1, left, stack, Position::Schema)?;
                } else {
                    let child = match position {
                        Position::Map => Position::Schema,
                        Position::Schema => match key.as_str() {
                            "$id" | "id" if depth > 0 => {
                                anyhow::bail!("Nested schema identifiers are unsupported")
                            }
                            "properties" | "patternProperties" | "$defs" | "definitions"
                            | "dependentSchemas" | "dependencies" => Position::Map,
                            "allOf" | "anyOf" | "oneOf" | "prefixItems" => Position::Array,
                            "items" if value.is_array() => Position::Array,
                            "items"
                            | "additionalProperties"
                            | "additionalItems"
                            | "not"
                            | "contains"
                            | "propertyNames"
                            | "if"
                            | "then"
                            | "else"
                            | "unevaluatedProperties"
                            | "unevaluatedItems"
                            | "contentSchema" => Position::Schema,
                            _ => Position::Data,
                        },
                        _ => Position::Data,
                    };
                    budget(root, value, depth + 1, left, stack, child)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                budget(
                    root,
                    value,
                    depth + 1,
                    left,
                    stack,
                    if matches!(position, Position::Array) {
                        Position::Schema
                    } else {
                        Position::Data
                    },
                )?;
            }
        }
        _ => (),
    }
    stack.pop();
    Ok(())
}
fn instance_budget(instance: &Value, depth: usize, left: &mut usize) -> Result<()> {
    ensure!(
        depth <= 128 && *left > 0,
        "Schema instance complexity limit exceeded"
    );
    *left -= 1;
    match instance {
        Value::Object(map) => {
            for value in map.values() {
                instance_budget(value, depth + 1, left)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                instance_budget(value, depth + 1, left)?;
            }
        }
        _ => (),
    };
    Ok(())
}
pub fn validate(schema: &str, instance: &Value) -> Result<Vec<String>> {
    ensure!(schema.len() <= 256 * 1024, "Schema exceeds 256 KiB");
    let schema: Value =
        serde_json::from_str(schema).map_err(|_| anyhow::anyhow!("Invalid JSON Schema JSON"))?;
    budget(
        &schema,
        &schema,
        0,
        &mut 2048,
        &mut vec![],
        Position::Schema,
    )?;
    instance_budget(instance, 0, &mut 100_000)?;
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .with_pattern_options(
            jsonschema::PatternOptions::regex()
                .size_limit(1024 * 1024)
                .dfa_size_limit(1024 * 1024),
        )
        .build(&schema)
        .map_err(|_| anyhow::anyhow!("Invalid or unsupported JSON Schema"))?;
    Ok(validator
        .iter_errors(instance)
        .take(10)
        .map(|error| {
            format!(
                "Schema mismatch at {}",
                crate::utils::truncate_str(&error.instance_path().to_string(), 256)
            )
        })
        .collect())
}
