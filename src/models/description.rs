use serde_json::Value;
use std::collections::BTreeMap;

pub type DescriptionMetadata = BTreeMap<String, Value>;

pub fn description_supported(value: &Value) -> bool {
    value.is_null() || value.is_string() || value.get("content").is_some_and(Value::is_string)
}

pub fn decode_description(value: &Value) -> (String, Option<DescriptionMetadata>) {
    if let Some(text) = value.as_str() {
        return (text.into(), None);
    }
    if let Some(text) = value.get("content").and_then(Value::as_str) {
        let metadata = value.as_object().map(|object| {
            object
                .iter()
                .filter(|(key, _)| key.as_str() != "content")
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        });
        return (text.into(), metadata);
    }
    (String::new(), None)
}

pub fn encode_description(text: &str, metadata: &Option<DescriptionMetadata>) -> Value {
    match metadata {
        Some(metadata) => {
            let mut value: serde_json::Map<String, Value> = metadata.clone().into_iter().collect();
            value.insert("content".into(), text.into());
            Value::Object(value)
        }
        None => text.into(),
    }
}
