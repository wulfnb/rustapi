use super::*;
use std::collections::BTreeMap;
impl Collection {
    /// Origins and secret type hints for active collection/folder variables.
    pub fn variable_origins(&self, request_id: &str) -> BTreeMap<String, (String, bool)> {
        fn add(
            values: &std::collections::HashMap<String, String>,
            rows: &[KeyValue],
            name: String,
            map: &mut BTreeMap<String, (String, bool)>,
        ) {
            for key in effective_scope_variables(values, rows).keys() {
                let secret = rows.iter().any(|row| {
                    row.enabled && row.key == *key && row.value_type.eq_ignore_ascii_case("secret")
                });
                map.insert(key.clone(), (name.clone(), secret));
            }
        }
        fn find(
            folders: &[Folder],
            id: &str,
            origins: &BTreeMap<String, (String, bool)>,
        ) -> Option<BTreeMap<String, (String, bool)>> {
            for folder in folders {
                let mut next = origins.clone();
                add(
                    &folder.variables,
                    &folder.variable_metadata,
                    format!("Folder: {}", folder.name),
                    &mut next,
                );
                if folder.requests.iter().any(|r| r.id == id) {
                    return Some(next);
                }
                if let Some(found) = find(&folder.folders, id, &next) {
                    return Some(found);
                }
            }
            None
        }
        let mut origins = BTreeMap::new();
        add(
            &self.variables,
            &self.variable_metadata,
            format!("Collection: {}", self.name),
            &mut origins,
        );
        if self.requests.iter().any(|r| r.id == request_id) {
            origins
        } else {
            find(&self.folders, request_id, &origins).unwrap_or_default()
        }
    }
}
