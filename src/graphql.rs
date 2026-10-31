//! Dedicated GraphQL document/variables handling shared by native HTTP and exports.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Graphql {
    pub query: String,
    pub variables: String,
    pub operation_name: String,
    pub schema: String,
}
impl Default for Graphql {
    fn default() -> Self {
        Self {
            query: "query {\n  __typename\n}".into(),
            variables: "{}".into(),
            operation_name: String::new(),
            schema: String::new(),
        }
    }
}
fn budget(query: &str) -> Result<()> {
    ensure!(
        query.len() <= 256 * 1024,
        "GraphQL document exceeds 256 KiB"
    );
    let bytes = query.as_bytes();
    let (mut i, mut depth, mut string, mut block, mut comment) = (0, 0usize, false, false, false);
    while i < bytes.len() {
        let b = bytes[i];
        if comment {
            if b == b'\n' {
                comment = false;
            }
            i += 1;
            continue;
        }
        if block {
            if bytes.get(i..i + 3) == Some(b"\"\"\"") && (i == 0 || bytes[i - 1] != b'\\') {
                block = false;
                i += 3;
            } else {
                i += 1;
            }
            continue;
        }
        if string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                string = false;
            }
            i += 1;
            continue;
        }
        if bytes.get(i..i + 3) == Some(b"\"\"\"") {
            block = true;
            i += 3;
            continue;
        }
        match b {
            b'#' => comment = true,
            b'"' => string = true,
            b'{' | b'[' | b'(' => {
                depth += 1;
                ensure!(depth <= 64, "GraphQL nesting exceeds 64 levels");
            }
            b'}' | b']' | b')' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}
impl Graphql {
    pub fn body(&self) -> Result<String> {
        use graphql_parser::query::{Definition, OperationDefinition};
        budget(&self.query)?;
        let document = graphql_parser::parse_query::<String>(&self.query)
            .map_err(|_| anyhow::anyhow!("Invalid GraphQL syntax"))?;
        let operations = document
            .definitions
            .iter()
            .filter_map(|definition| {
                if let Definition::Operation(operation) = definition {
                    Some(operation)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        ensure!(!operations.is_empty(), "GraphQL document has no operation");
        ensure!(
            operations.len() == 1 || !self.operation_name.is_empty(),
            "Select an operation name for a document with multiple operations"
        );
        let selected = operations
            .iter()
            .find(|operation| {
                let name = match operation {
                    OperationDefinition::Query(q) => q.name.as_deref(),
                    OperationDefinition::Mutation(m) => m.name.as_deref(),
                    OperationDefinition::Subscription(s) => s.name.as_deref(),
                    _ => None,
                };
                self.operation_name.is_empty() || name == Some(self.operation_name.as_str())
            })
            .context("GraphQL operation name was not found")?;
        ensure!(
            !matches!(selected, OperationDefinition::Subscription(_)),
            "GraphQL subscriptions require a subscription transport"
        );
        ensure!(
            self.variables.len() <= 1024 * 1024,
            "GraphQL variables exceed 1 MiB"
        );
        let variables: serde_json::Value = serde_json::from_str(&self.variables)
            .context("GraphQL variables must be valid JSON")?;
        ensure!(
            variables.is_object(),
            "GraphQL variables must be a JSON object"
        );
        let mut body = serde_json::json!({"query":self.query,"variables":variables});
        if !self.operation_name.is_empty() {
            body["operationName"] = self.operation_name.clone().into();
        }
        Ok(body.to_string())
    }
    pub fn format(&mut self) -> Result<()> {
        budget(&self.query)?;
        self.query = graphql_parser::parse_query::<String>(&self.query)
            .map_err(|_| anyhow::anyhow!("Invalid GraphQL syntax"))?
            .to_string();
        self.variables = serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(
            &self.variables,
        )?)?;
        Ok(())
    }
    pub fn fields(&self) -> Result<Vec<String>> {
        ensure!(
            self.schema.len() <= 4 * 1024 * 1024,
            "GraphQL schema exceeds 4 MiB"
        );
        if self.schema.trim_start().starts_with('{') {
            let value: serde_json::Value = serde_json::from_str(&self.schema)?;
            let schema = value.get("data").unwrap_or(&value);
            let types = schema["__schema"]["types"]
                .as_array()
                .context("Introspection response has no schema types")?;
            Ok(types
                .iter()
                .flat_map(|t| {
                    t["fields"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(move |field| {
                            format!(
                                "{}.{}",
                                t["name"].as_str().unwrap_or_default(),
                                field["name"].as_str().unwrap_or_default()
                            )
                        })
                })
                .collect())
        } else {
            budget(&self.schema)?;
            let document = graphql_parser::parse_schema::<String>(&self.schema)
                .map_err(|_| anyhow::anyhow!("Invalid GraphQL schema syntax"))?;
            Ok(document
                .definitions
                .iter()
                .flat_map(|d| match d {
                    graphql_parser::schema::Definition::TypeDefinition(
                        graphql_parser::schema::TypeDefinition::Object(object),
                    ) => object
                        .fields
                        .iter()
                        .map(|f| format!("{}.{}", object.name, f.name))
                        .collect::<Vec<_>>(),
                    _ => vec![],
                })
                .collect())
        }
    }
}
pub const INTROSPECTION:&str="query RustAPIIntrospection { __schema { queryType { name } mutationType { name } subscriptionType { name } types { kind name description fields(includeDeprecated:true) { name description args { name defaultValue type { kind name ofType { kind name ofType { kind name } } } } type { kind name ofType { kind name ofType { kind name } } } } } } }";
pub fn prepare(request: &mut crate::models::ApiRequest) -> Result<()> {
    if let Some(graphql) = &request.graphql {
        request.body = graphql.body()?;
        request.body_type = crate::models::BodyType::Json;
        request.method = crate::models::HttpMethod::POST;
    }
    Ok(())
}
