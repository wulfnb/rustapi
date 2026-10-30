//! Collection and Folder models

use super::request::ApiRequest;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A folder containing requests
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Folder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_metadata: Option<super::DescriptionMetadata>,
    pub requests: Vec<ApiRequest>,
    #[serde(default)]
    pub order: Vec<String>,
    pub folders: Vec<Folder>,
    pub expanded: bool,
    #[serde(default)]
    pub auth: super::AuthConfig,
    #[serde(default)]
    pub pre_script: String,
    #[serde(default)]
    pub post_script: String,
    #[serde(default)]
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub variable_metadata: Vec<super::KeyValue>,
}

impl Folder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: String::new(),
            description_metadata: None,
            requests: vec![],
            order: vec![],
            folders: vec![],
            expanded: true,
            auth: super::AuthConfig {
                auth_type: super::AuthType::Inherit,
                ..Default::default()
            },
            pre_script: String::new(),
            post_script: String::new(),
            variables: HashMap::new(),
            variable_metadata: vec![],
        }
    }
}

/// A collection of API requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_metadata: Option<super::DescriptionMetadata>,
    pub version: String,
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub auth: super::AuthConfig,
    #[serde(default)]
    pub pre_script: String,
    #[serde(default)]
    pub post_script: String,
    pub requests: Vec<ApiRequest>,
    #[serde(default)]
    pub order: Vec<String>,
    pub folders: Vec<Folder>,
    pub variables: HashMap<String, String>,
    #[serde(default)]
    pub variable_metadata: Vec<super::KeyValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub file_path: Option<String>,
    #[serde(skip)]
    pub expanded: bool,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::from("New Collection"),
            description: String::new(),
            description_metadata: None,
            version: String::from("1.0.0"),
            schema_version: schema_version(),
            auth: Default::default(),
            pre_script: String::new(),
            post_script: String::new(),
            requests: vec![],
            order: vec![],
            folders: vec![],
            variables: HashMap::new(),
            variable_metadata: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            file_path: None,
            expanded: true,
        }
    }
}

impl Collection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

impl Collection {
    pub fn request(&self, id: &str) -> Option<&ApiRequest> {
        find_request(&self.requests, &self.folders, id)
    }
    pub fn request_mut(&mut self, id: &str) -> Option<&mut ApiRequest> {
        find_request_mut(&mut self.requests, &mut self.folders, id)
    }
    pub fn folder_mut(&mut self, id: &str) -> Option<&mut Folder> {
        find_folder_mut(&mut self.folders, id)
    }
    pub fn remove_request(&mut self, id: &str) -> Option<ApiRequest> {
        remove_request(&mut self.requests, &mut self.folders, id)
    }
    pub fn remove_folder(&mut self, id: &str) -> Option<Folder> {
        remove_folder(&mut self.folders, id)
    }
    pub fn duplicate_request(&mut self, id: &str) -> Option<ApiRequest> {
        fn duplicate(
            requests: &mut Vec<ApiRequest>,
            folders: &mut [Folder],
            order: &mut Vec<String>,
            id: &str,
        ) -> Option<ApiRequest> {
            if let Some(r) = requests.iter().find(|r| r.id == id) {
                let copy = r.duplicate();
                requests.push(copy.clone());
                if let Some(i) = order.iter().position(|v| v == id) {
                    order.insert(i + 1, copy.id.clone());
                }
                return Some(copy);
            }
            folders
                .iter_mut()
                .find_map(|f| duplicate(&mut f.requests, &mut f.folders, &mut f.order, id))
        }
        duplicate(&mut self.requests, &mut self.folders, &mut self.order, id)
    }
    pub fn ordered_requests(&self, folder: Option<&str>) -> Vec<ApiRequest> {
        fn visit(
            requests: &[ApiRequest],
            folders: &[Folder],
            order: &[String],
            scope: (&HashMap<String, String>, &super::AuthConfig, &str, &str),
            out: &mut Vec<ApiRequest>,
        ) {
            let (vars, auth, pre, post) = scope;
            for id in child_order(requests, folders, order) {
                if let Some(r) = requests.iter().find(|r| r.id == id) {
                    let mut r = r.clone();
                    if r.auth.auth_type == super::AuthType::Inherit {
                        r.auth = auth.clone();
                    }
                    r.pre_script = format!("{pre}\n{}", r.pre_script);
                    r.post_script = format!("{post}\n{}", r.post_script);
                    // Folder variables are materialized before narrower environment/run scopes.
                    r.inherited_variables = vars.clone();
                    out.push(r);
                } else if let Some(f) = folders.iter().find(|f| f.id == id) {
                    let mut vars = vars.clone();
                    vars.extend(effective_scope_variables(
                        &f.variables,
                        &f.variable_metadata,
                    ));
                    let auth = if f.auth.auth_type == super::AuthType::Inherit {
                        auth
                    } else {
                        &f.auth
                    };
                    visit(
                        &f.requests,
                        &f.folders,
                        &f.order,
                        (
                            &vars,
                            auth,
                            &format!("{pre}\n{}", f.pre_script),
                            &format!("{post}\n{}", f.post_script),
                        ),
                        out,
                    );
                }
            }
        }
        let mut all = vec![];
        let variables = effective_scope_variables(&self.variables, &self.variable_metadata);
        visit(
            &self.requests,
            &self.folders,
            &self.order,
            (&variables, &self.auth, &self.pre_script, &self.post_script),
            &mut all,
        );
        if let Some(id) = folder {
            let mut copy = self.clone();
            if let Some(f) = copy.folder_mut(id) {
                fn ids(f: &Folder, out: &mut Vec<String>) {
                    out.extend(f.requests.iter().map(|r| r.id.clone()));
                    for c in &f.folders {
                        ids(c, out);
                    }
                }
                let mut selected = vec![];
                ids(f, &mut selected);
                all.retain(|r| selected.contains(&r.id));
            } else {
                all.clear();
            }
        }
        if let Some(path) = &self.file_path {
            let path = std::path::Path::new(path);
            let parent = path.parent().unwrap_or(std::path::Path::new("."));
            let base = if parent.file_name().is_some_and(|n| n == "collections") {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            };
            for r in &mut all {
                r.file_base = base.into();
            }
        }
        all
    }
}
pub fn find_request<'a>(
    requests: &'a [ApiRequest],
    folders: &'a [Folder],
    id: &str,
) -> Option<&'a ApiRequest> {
    requests.iter().find(|r| r.id == id).or_else(|| {
        folders
            .iter()
            .find_map(|f| find_request(&f.requests, &f.folders, id))
    })
}
pub fn find_request_mut<'a>(
    requests: &'a mut [ApiRequest],
    folders: &'a mut [Folder],
    id: &str,
) -> Option<&'a mut ApiRequest> {
    if let Some(r) = requests.iter_mut().find(|r| r.id == id) {
        return Some(r);
    }
    folders
        .iter_mut()
        .find_map(|f| find_request_mut(&mut f.requests, &mut f.folders, id))
}
fn find_folder_mut<'a>(folders: &'a mut [Folder], id: &str) -> Option<&'a mut Folder> {
    for f in folders {
        if f.id == id {
            return Some(f);
        }
        if let Some(c) = find_folder_mut(&mut f.folders, id) {
            return Some(c);
        }
    }
    None
}
fn remove_request(
    requests: &mut Vec<ApiRequest>,
    folders: &mut [Folder],
    id: &str,
) -> Option<ApiRequest> {
    if let Some(i) = requests.iter().position(|r| r.id == id) {
        return Some(requests.remove(i));
    }
    folders
        .iter_mut()
        .find_map(|f| remove_request(&mut f.requests, &mut f.folders, id))
}
fn remove_folder(folders: &mut Vec<Folder>, id: &str) -> Option<Folder> {
    if let Some(i) = folders.iter().position(|f| f.id == id) {
        return Some(folders.remove(i));
    }
    folders
        .iter_mut()
        .find_map(|f| remove_folder(&mut f.folders, id))
}

fn schema_version() -> u32 {
    1
}

pub fn child_order(requests: &[ApiRequest], folders: &[Folder], order: &[String]) -> Vec<String> {
    let mut ids = vec![];
    for id in order
        .iter()
        .chain(requests.iter().map(|r| &r.id))
        .chain(folders.iter().map(|f| &f.id))
    {
        if !ids.contains(id)
            && (requests.iter().any(|r| &r.id == id) || folders.iter().any(|f| &f.id == id))
        {
            ids.push(id.clone());
        }
    }
    ids
}

pub fn validate_collection(c: &Collection) -> anyhow::Result<()> {
    anyhow::ensure!(
        c.schema_version == 1,
        "Unsupported collection schema {}",
        c.schema_version
    );
    fn visit(
        requests: &[ApiRequest],
        folders: &[Folder],
        seen: &mut std::collections::HashSet<String>,
    ) -> anyhow::Result<()> {
        for r in requests {
            anyhow::ensure!(
                !r.id.is_empty() && seen.insert(r.id.clone()),
                "Empty or duplicate request ID"
            );
        }
        for f in folders {
            anyhow::ensure!(
                !f.id.is_empty() && seen.insert(f.id.clone()),
                "Empty or duplicate folder ID"
            );
            visit(&f.requests, &f.folders, seen)?;
        }
        Ok(())
    }
    visit(&c.requests, &c.folders, &mut Default::default())
}

/// Legacy maps remain authoritative for values; metadata supplies disabled flags.
pub fn effective_scope_variables(
    values: &HashMap<String, String>,
    rows: &[super::KeyValue],
) -> HashMap<String, String> {
    let mut values = values.clone();
    let enabled: std::collections::HashSet<_> =
        rows.iter().filter(|r| r.enabled).map(|r| &r.key).collect();
    for row in rows {
        if !row.enabled && !enabled.contains(&row.key) {
            values.remove(&row.key);
        }
    }
    values
}

/// Reconstruct portable rows without resurrecting values deleted by scripts or edits.
pub fn scope_variable_rows(
    values: &HashMap<String, String>,
    rows: &[super::KeyValue],
) -> Vec<super::KeyValue> {
    let mut output = vec![];
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let mut row = row.clone();
        if row.enabled {
            if let Some(value) = values.get(&row.key) {
                row.value = value.clone();
            } else {
                row.enabled = false;
            }
        }
        seen.insert(row.key.clone());
        output.push(row);
    }
    let mut extra: Vec<_> = values
        .iter()
        .filter(|(key, _)| !seen.contains(*key))
        .collect();
    extra.sort_by_key(|(key, _)| *key);
    output.extend(
        extra
            .into_iter()
            .map(|(key, value)| super::KeyValue::new(key, value)),
    );
    output
}
