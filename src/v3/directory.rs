use super::{mapping, parse_yaml};
use crate::{
    import::{ImportDiagnostic, ImportDisposition, ImportResult, ImportSeverity},
    models::*,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
const SIDECAR: &str = ".resources/rustapi.json";
#[derive(Serialize, Deserialize)]
struct NativeSidecar {
    version: u32,
    collection: Collection,
}
fn relative(path: &Path) -> Result<String> {
    let mut parts = vec![];
    for part in path.components() {
        match part {
            std::path::Component::Normal(part) => {
                let part = part.to_str().context("Non UTF-8 v3 path")?;
                ensure!(!part.contains('\\'), "Use forward-slash paths");
                parts.push(part);
            }
            _ => anyhow::bail!("Invalid v3 relative path"),
        }
    }
    Ok(parts.join("/"))
}
fn collect(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, Vec<u8>>,
    total: &mut u64,
    depth: usize,
) -> Result<()> {
    ensure!(depth <= 32, "v3 directory exceeds 32 levels");
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .to_str()
            .context("Non UTF-8 v3 path")?
            .to_owned();
        ensure!(
            names.insert(name.to_lowercase()),
            "Case-insensitive path collision"
        );
        ensure!(!name.contains('\\'), "Use forward-slash paths");
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "v3 resources cannot use symlinks"
        );
        if metadata.is_dir() {
            collect(root, &entry.path(), files, total, depth + 1)?;
        } else if metadata.is_file() {
            ensure!(files.len() < 1000, "v3 directory exceeds 1000 files");
            *total = total
                .checked_add(metadata.len())
                .context("v3 size overflow")?;
            ensure!(*total <= 64 * 1024 * 1024, "v3 directory exceeds 64 MiB");
            let relative = relative(entry.path().strip_prefix(root)?)?;
            let limit = if relative == SIDECAR {
                64 * 1024 * 1024
            } else {
                8 * 1024 * 1024
            };
            ensure!(metadata.len() <= limit, "v3 resource exceeds size limit");
            files.insert(relative, fs::read(entry.path())?);
        } else {
            anyhow::bail!("Unsupported v3 filesystem entry");
        }
    }
    Ok(())
}
fn load(
    files: &BTreeMap<String, Vec<u8>>,
    path: &str,
    used: &mut BTreeSet<String>,
) -> Result<Value> {
    used.insert(path.into());
    parse_yaml(std::str::from_utf8(
        files.get(path).context("Missing v3 YAML resource")?,
    )?)
}
fn pick(files: &BTreeMap<String, Vec<u8>>, candidates: &[String]) -> Result<Option<String>> {
    let matches: Vec<_> = candidates
        .iter()
        .filter(|path| files.contains_key(*path))
        .cloned()
        .collect();
    ensure!(matches.len() <= 1, "Ambiguous v3 resource definitions");
    Ok(matches.into_iter().next())
}
fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.into()
    } else {
        format!("{prefix}/{name}")
    }
}
fn ordered(value: &Value) -> Result<f64> {
    if value["order"].is_null() {
        Ok(0.)
    } else {
        value["order"]
            .as_f64()
            .filter(|number| number.is_finite())
            .context("order must be a finite number")
    }
}
fn scan(
    files: &BTreeMap<String, Vec<u8>>,
    prefix: &str,
    used: &mut BTreeSet<String>,
) -> Result<Value> {
    let definition = pick(
        files,
        &[
            join(prefix, ".resources/definition.yaml"),
            join(prefix, ".resources/definition.yml"),
            join(prefix, "definition.yaml"),
            join(prefix, "definition.yml"),
        ],
    )?;
    let name = prefix
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Imported collection");
    let mut result = if let Some(path) = definition {
        load(files, &path, used)?
    } else {
        json!({"$kind":"collection","name":name})
    };
    ensure!(
        result["$kind"] == "collection",
        "Directory metadata must be a collection"
    );
    if result["name"].is_null() {
        result["name"] = name.into();
    }
    ensure!(
        result["items"].is_null(),
        "Directory definitions cannot also contain inline items"
    );
    let mut entries = BTreeSet::new();
    for path in files.keys() {
        let rest = if prefix.is_empty() {
            path.as_str()
        } else {
            let Some(rest) = path.strip_prefix(&format!("{prefix}/")) else {
                continue;
            };
            rest
        };
        if let Some(entry) = rest.split('/').next() {
            if entry != ".resources"
                && !entry.ends_with(".resources")
                && entry != "definition.yaml"
                && entry != "definition.yml"
            {
                entries.insert(entry.to_owned());
            }
        }
    }
    let mut children = vec![];
    let mut logical_names = BTreeSet::new();
    for entry in entries {
        let path = join(prefix, &entry);
        if entry.ends_with(".request.yaml") || entry.ends_with(".request.yml") {
            let mut request = load(files, &path, used)?;
            ensure!(
                request["$kind"] == "http-request" || request["$kind"] == "graphql-request",
                "Only v3 HTTP and GraphQL request files are supported"
            );
            let stem = entry
                .strip_suffix(".request.yaml")
                .or_else(|| entry.strip_suffix(".request.yml"))
                .unwrap();
            if request["name"].is_null() {
                request["name"] = stem.into();
            }
            ensure!(
                logical_names.insert(stem.to_lowercase()),
                "Ambiguous request filename stems"
            );
            let resource_roots = [
                join(prefix, &format!("{stem}.resources/examples")),
                join(prefix, &format!(".resources/{stem}.resources/examples")),
            ];
            let mut examples: Vec<(f64, String, Value)> = request["examples"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .enumerate()
                .map(|(i, value)| Ok((ordered(&value)?, format!("inline-{i}"), value)))
                .collect::<Result<_>>()?;
            for (file, _) in files.iter().filter(|(file, _)| {
                resource_roots
                    .iter()
                    .any(|root| file.starts_with(&format!("{root}/")))
            }) {
                ensure!(
                    file.ends_with(".example.yaml") || file.ends_with(".example.yml"),
                    "Unsupported example resource file"
                );
                let mut example = load(files, file, used)?;
                if example["name"].is_null() {
                    example["name"] = Path::new(file)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .trim_end_matches(".yaml")
                        .trim_end_matches(".yml")
                        .trim_end_matches(".example")
                        .into();
                }
                examples.push((ordered(&example)?, file.clone(), example));
            }
            examples.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            request["examples"] =
                Value::Array(examples.into_iter().map(|(_, _, value)| value).collect());
            children.push((ordered(&request)?, entry, request));
        } else if files
            .keys()
            .any(|file| file.starts_with(&format!("{path}/")))
        {
            let folder = scan(files, &path, used)?;
            children.push((ordered(&folder)?, entry, folder));
        } else {
            anyhow::bail!("Unsupported file in v3 collection directory");
        }
    }
    children.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    result["items"] = Value::Array(children.into_iter().map(|(_, _, value)| value).collect());
    Ok(result)
}
fn stem(name: &str, index: usize) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(70)
        .collect();
    format!(
        "{:04}-{}",
        index + 1,
        if name.is_empty() { "item" } else { &name }
    )
}
fn flatten(mut value: Value, prefix: &str, output: &mut BTreeMap<String, Value>) -> Result<()> {
    let children = value
        .as_object_mut()
        .context("v3 object")?
        .remove("items")
        .unwrap_or(json!([]));
    output.insert(join(prefix, ".resources/definition.yaml"), value);
    for (index, mut child) in children
        .as_array()
        .context("v3 items")?
        .iter()
        .cloned()
        .enumerate()
    {
        let name = stem(child["name"].as_str().unwrap_or("item"), index);
        child["order"] = json!((index + 1) * 1000);
        if child["$kind"] == "collection" {
            flatten(child, &join(prefix, &name), output)?;
        } else {
            let examples = child
                .as_object_mut()
                .unwrap()
                .remove("examples")
                .unwrap_or(json!([]));
            output.insert(join(prefix, &format!("{name}.request.yaml")), child);
            for (i, mut example) in examples.as_array().unwrap().iter().cloned().enumerate() {
                let example_name = stem(example["name"].as_str().unwrap_or("example"), i);
                example["order"] = json!((i + 1) * 1000);
                output.insert(
                    join(
                        prefix,
                        &format!("{name}.resources/examples/{example_name}.example.yaml"),
                    ),
                    example,
                );
            }
        }
    }
    Ok(())
}
fn build(collection: &Collection) -> Result<BTreeMap<String, Value>> {
    let portable = crate::export::collection(collection, true)?;
    let mut output = BTreeMap::new();
    flatten(mapping::from_21(&portable.document, true), "", &mut output)?;
    Ok(output)
}
pub fn import_directory(root: &Path) -> Result<ImportResult> {
    ensure!(
        !fs::symlink_metadata(root)?.file_type().is_symlink(),
        "v3 root cannot be a symlink"
    );
    let root = root.canonicalize()?;
    let mut files = BTreeMap::new();
    collect(&root, &root, &mut files, &mut 0, 0)?;
    let mut used = BTreeSet::new();
    let document = scan(&files, "", &mut used)?;
    let mut imported = super::import_document(&document)?;
    if let Some(bytes) = files.get(SIDECAR) {
        used.insert(SIDECAR.into());
        let native: NativeSidecar = serde_json::from_slice(bytes)?;
        ensure!(native.version == 1, "Unsupported v3 native sidecar version");
        validate_collection(&native.collection)?;
        let expected = build(&native.collection)?;
        let actual = used
            .iter()
            .filter(|path| path.as_str() != SIDECAR)
            .map(|path| {
                Ok((
                    path.clone(),
                    parse_yaml(std::str::from_utf8(&files[path])?)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        if expected == actual {
            let mut collection = native.collection;
            collection.id = uuid::Uuid::new_v4().to_string();
            collection.file_path = None;
            imported.collection = Some(collection);
            imported.warnings.push(
                "Verified native sidecar restored fields other v3 consumers may ignore".into(),
            );
        } else {
            imported.diagnostics.push(ImportDiagnostic{path:"/.resources/rustapi.json".into(),feature:"stale_native_metadata".into(),severity:ImportSeverity::Warning,disposition:ImportDisposition::Omitted,message:"YAML changed after export; stale native metadata was not applied. Standard YAML fields remain importable.".into()});
        }
    }
    ensure!(
        used.len() == files.len(),
        "Unrecognized v3 resource files; import stopped without dropping them"
    );
    Ok(imported)
}
pub fn export_directory(
    collection: &Collection,
    target: &Path,
    include_credentials: bool,
) -> Result<Vec<String>> {
    ensure!(
        !target.exists(),
        "v3 export requires a new destination directory"
    );
    validate_collection(collection)?;
    let collection = if include_credentials {
        collection.clone()
    } else {
        crate::secrets::redact_collection(collection)
    };
    let files = build(&collection)?;
    ensure!(files.len() < 1000, "v3 export exceeds 1000 files");
    let mut encoded = BTreeMap::new();
    for (path, value) in files {
        let bytes = serde_saphyr::to_string(&value)?.into_bytes();
        ensure!(bytes.len() <= 8 * 1024 * 1024, "YAML file exceeds 8 MiB");
        encoded.insert(path, bytes);
    }
    encoded.insert(
        SIDECAR.into(),
        serde_json::to_vec_pretty(&NativeSidecar {
            version: 1,
            collection,
        })?,
    );
    ensure!(
        encoded.values().map(Vec::len).sum::<usize>() <= 64 * 1024 * 1024,
        "v3 export exceeds 64 MiB"
    );
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = tempfile::tempdir_in(parent)?;
    for (path, bytes) in encoded {
        let path: PathBuf = temporary.path().join(path);
        crate::workspace::atomic_write(&path, &bytes)?;
    }
    ensure!(!target.exists(), "v3 destination already exists");
    fs::rename(temporary.path(), target)?;
    Ok(vec!["Native metadata is stored in .resources/rustapi.json; other consumers use standard YAML fields. Editing YAML requires review before stale native metadata is omitted.".into()])
}
