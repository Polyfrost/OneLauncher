use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::version::{Predicate, Version, maven_range, maven_version};

pub type Ranges = BTreeMap<String, Vec<Predicate>>;

#[derive(Debug, Clone)]
pub struct ModMeta {
    pub id: String,
    pub version: Version,
    pub provides: Vec<String>,
    pub depends: Ranges,
    pub breaks: Ranges,
    pub recommends: Ranges,
    pub suggests: Ranges,
    pub conflicts: Ranges,
    pub nested: Vec<ModMeta>,
    pub loose_id: bool,
}

impl ModMeta {
    pub(super) fn library(file_name: &str) -> Self {
        let stem = file_name
            .trim_end_matches(".disabled")
            .trim_end_matches(".jar");
        Self::bare(format!("library-{}", stem.to_ascii_lowercase()))
    }

    fn bare(id: String) -> Self {
        Self {
            id,
            version: Version::Unknown,
            provides: Vec::new(),
            depends: Ranges::new(),
            breaks: Ranges::new(),
            recommends: Ranges::new(),
            suggests: Ranges::new(),
            conflicts: Ranges::new(),
            nested: Vec::new(),
            loose_id: false,
        }
    }

    pub fn forget(&mut self, dep: &str) {
        self.depends.remove(dep);
        for nested in &mut self.nested {
            nested.depends.remove(dep);
        }
    }

    pub fn mentions(&self, id: &str) -> bool {
        [
            &self.depends,
            &self.breaks,
            &self.recommends,
            &self.suggests,
            &self.conflicts,
        ]
        .iter()
        .any(|ranges| ranges.contains_key(id))
    }
}

const FABRIC: &str = "fabric.mod.json";
const QUILT: &str = "quilt.mod.json";
const NEOFORGE: &str = "META-INF/neoforge.mods.toml";
const FORGE: &str = "META-INF/mods.toml";
const LEGACY_FORGE: &str = "mcmod.info";
const JAR_MANIFEST: &str = "META-INF/MANIFEST.MF";
const MANIFESTS: [&str; 5] = [FABRIC, QUILT, NEOFORGE, FORGE, LEGACY_FORGE];
const NESTED_DIRS: [&str; 2] = ["META-INF/jars/", "META-INF/jarjar/"];
const MAX_NESTED_DEPTH: usize = 4;

fn wanted(name: &str) -> bool {
    MANIFESTS.contains(&name) || name == JAR_MANIFEST || is_nested(name)
}

fn is_nested(name: &str) -> bool {
    name.ends_with(".jar") && NESTED_DIRS.iter().any(|dir| name.starts_with(dir))
}

pub async fn read_mod(path: &Path, file_name: &str) -> Option<ModMeta> {
    let entries = polyio::read_zip_file_entries(path, wanted).await.ok()?;
    Some(from_entries(entries, file_name, 0).await)
}

async fn from_entries(entries: Vec<(String, Vec<u8>)>, file_name: &str, depth: usize) -> ModMeta {
    let text = |wanted: &str| {
        entries
            .iter()
            .find(|(name, _)| name == wanted)
            .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
    };
    let jar_version = text(JAR_MANIFEST).and_then(|manifest| {
        manifest.lines().find_map(|line| {
            line.strip_prefix("Implementation-Version:")
                .map(|v| v.trim().to_string())
        })
    });

    let parsed = MANIFESTS.iter().find_map(|manifest| {
        let body = text(manifest)?;
        match *manifest {
            FABRIC => parse_fabric(body),
            QUILT => parse_quilt(body),
            NEOFORGE | FORGE => parse_forge(body, jar_version.as_deref()),
            _ => parse_legacy(body),
        }
    });
    let mut meta = parsed.unwrap_or_else(|| ModMeta::library(file_name));

    if depth < MAX_NESTED_DEPTH {
        for (name, bytes) in entries {
            if !is_nested(&name) {
                continue;
            }
            let Ok(inner) = polyio::read_zip_bytes_entries(bytes, wanted).await else {
                continue;
            };
            let nested_name = name.rsplit('/').next().unwrap_or(&name);
            let mut nested = Box::pin(from_entries(inner, nested_name, depth + 1)).await;
            let deeper = std::mem::take(&mut nested.nested);
            meta.nested.push(nested);
            meta.nested.extend(deeper);
        }
    }
    meta
}

fn id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn fabric_ranges(value: Option<&serde_json::Value>) -> Ranges {
    let Some(map) = value.and_then(|v| v.as_object()) else {
        return Ranges::new();
    };
    map.iter()
        .map(|(dep, spec)| {
            let predicates: Vec<Predicate> = match spec {
                serde_json::Value::String(s) => {
                    vec![Predicate::parse(s).unwrap_or_else(Predicate::any)]
                }
                serde_json::Value::Array(items) => items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(|s| Predicate::parse(s).unwrap_or_else(Predicate::any))
                    .collect(),
                _ => vec![Predicate::any()],
            };
            let predicates = if predicates.is_empty() {
                vec![Predicate::any()]
            } else {
                predicates
            };
            (id(dep), predicates)
        })
        .collect()
}

fn parse_fabric(text: &str) -> Option<ModMeta> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let mod_id = json.get("id")?.as_str()?;
    let mut meta = ModMeta::bare(id(mod_id));
    if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
        meta.version = Version::parse(version);
    }
    meta.provides = json
        .get("provides")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(id))
        .collect();
    meta.depends = fabric_ranges(json.get("depends"));
    meta.breaks = fabric_ranges(json.get("breaks"));
    meta.recommends = fabric_ranges(json.get("recommends"));
    meta.suggests = fabric_ranges(json.get("suggests"));
    meta.conflicts = fabric_ranges(json.get("conflicts"));
    Some(meta)
}

fn quilt_entry(value: &serde_json::Value) -> Option<(String, Vec<Predicate>, bool)> {
    match value {
        serde_json::Value::String(dep) => Some((id(dep), vec![Predicate::any()], false)),
        serde_json::Value::Object(map) => {
            let dep = id(map.get("id")?.as_str()?);
            let optional = map.get("optional").and_then(|v| v.as_bool()) == Some(true);
            let predicates = match map.get("versions") {
                Some(serde_json::Value::String(s)) => {
                    vec![Predicate::parse(s).unwrap_or_else(Predicate::any)]
                }
                Some(serde_json::Value::Array(items)) => items
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| Predicate::parse(s).unwrap_or_else(Predicate::any))
                    .collect(),
                _ => vec![Predicate::any()],
            };
            Some((dep, predicates, optional))
        }
        _ => None,
    }
}

fn parse_quilt(text: &str) -> Option<ModMeta> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let loader = json.get("quilt_loader")?;
    let mut meta = ModMeta::bare(id(loader.get("id")?.as_str()?));
    if let Some(version) = loader.get("version").and_then(|v| v.as_str()) {
        meta.version = Version::parse(version);
    }
    meta.provides = loader
        .get("provides")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| quilt_entry(v).map(|(dep, _, _)| dep))
        .collect();
    for (key, required, optional) in [("depends", true, false), ("breaks", false, false)] {
        for entry in loader
            .get(key)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(quilt_entry)
        {
            let (dep, predicates, is_optional) = entry;
            let target = match (required, is_optional || optional) {
                (true, false) => &mut meta.depends,
                (true, true) => &mut meta.recommends,
                (false, false) => &mut meta.breaks,
                (false, true) => &mut meta.conflicts,
            };
            target.insert(dep, predicates);
        }
    }
    Some(meta)
}

fn parse_forge(text: &str, jar_version: Option<&str>) -> Option<ModMeta> {
    let toml: toml::Value = toml::from_str(text).ok()?;
    let mods = toml.get("mods")?.as_array()?;
    let first = mods.first()?;
    let mut meta = ModMeta::bare(id(first.get("modId")?.as_str()?));
    let version = first.get("version").and_then(|v| v.as_str()).and_then(|v| {
        if v.contains("${file.jarVersion}") {
            jar_version.map(str::to_string)
        } else {
            Some(v.to_string())
        }
    });
    if let Some(version) = version.filter(|v| !v.contains("${")) {
        meta.version = Version::parse(&maven_version(&version));
    }
    meta.provides = mods
        .iter()
        .skip(1)
        .filter_map(|m| m.get("modId")?.as_str().map(id))
        .collect();

    let deps = toml
        .get("dependencies")
        .and_then(|v| v.as_table())
        .into_iter()
        .flat_map(|table| table.values())
        .filter_map(|v| v.as_array())
        .flatten();
    for dep in deps {
        let Some(dep_id) = dep.get("modId").and_then(|v| v.as_str()).map(id) else {
            continue;
        };
        if dep_id == meta.id || meta.provides.contains(&dep_id) {
            continue;
        }
        let kind = match dep.get("type").and_then(|v| v.as_str()) {
            Some(kind) => kind.to_ascii_lowercase(),
            None => match dep.get("mandatory").and_then(|v| v.as_bool()) {
                Some(false) => "optional".to_string(),
                _ => "required".to_string(),
            },
        };
        let predicates = maven_range(
            dep.get("versionRange")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
        );
        let target = match kind.as_str() {
            "required" => &mut meta.depends,
            "optional" => &mut meta.recommends,
            "incompatible" => &mut meta.breaks,
            "discouraged" => &mut meta.conflicts,
            _ => continue,
        };
        target.insert(dep_id, predicates);
    }
    Some(meta)
}

fn parse_legacy(text: &str) -> Option<ModMeta> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;
    let mods = json
        .as_array()
        .or_else(|| json.get("modList")?.as_array())?;
    let first = mods.first()?;
    let mut meta = ModMeta::bare(id(first.get("modid")?.as_str()?));
    meta.loose_id = true;
    if let Some(version) = first
        .get("version")
        .and_then(|v| v.as_str())
        .filter(|v| !v.contains("${"))
    {
        meta.version = Version::parse(&maven_version(version));
    }
    meta.provides = mods
        .iter()
        .skip(1)
        .filter_map(|m| m.get("modid")?.as_str().map(id))
        .collect();
    for spec in mods
        .iter()
        .filter_map(|m| m.get("requiredMods")?.as_array())
        .flatten()
        .filter_map(|v| v.as_str())
    {
        let (dep, range) = spec.split_once('@').unwrap_or((spec, ""));
        let dep = id(dep);
        if dep.is_empty() || dep == meta.id || meta.provides.contains(&dep) {
            continue;
        }
        meta.depends.insert(dep, maven_range(range));
    }
    Some(meta)
}

#[derive(Debug, Clone, Default)]
pub struct ClassIndex {
    pub declared: BTreeSet<String>,
    pub referenced: BTreeSet<String>,
}

pub async fn index_classes(path: &Path) -> ClassIndex {
    let mut index = ClassIndex::default();
    let entries = polyio::read_zip_file_entries(path, |name| {
        name.ends_with(".class") || name.ends_with(".jar")
    })
    .await
    .unwrap_or_default();
    index_entries(entries, &mut index, 0).await;
    index
}

async fn index_entries(entries: Vec<(String, Vec<u8>)>, index: &mut ClassIndex, depth: usize) {
    for (name, bytes) in entries {
        if name.ends_with(".class") {
            if let Some((declared, refs)) = parse_class(&bytes) {
                index.declared.insert(declared);
                index.referenced.extend(refs);
            }
        } else if depth < MAX_NESTED_DEPTH
            && let Ok(inner) = polyio::read_zip_bytes_entries(bytes, |name| {
                name.ends_with(".class") || name.ends_with(".jar")
            })
            .await
        {
            Box::pin(index_entries(inner, index, depth + 1)).await;
        }
    }
}

fn parse_class(data: &[u8]) -> Option<(String, Vec<String>)> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let slice = data.get(at..at + n)?;
        at += n;
        Some(slice)
    };
    let u2 = |bytes: &[u8]| -> Option<usize> {
        Some(u16::from_be_bytes(bytes.try_into().ok()?) as usize)
    };
    if take(4)? != [0xCA, 0xFE, 0xBA, 0xBE] {
        return None;
    }
    take(4)?;
    let count = u2(take(2)?)?;
    let mut utf8: Vec<Option<String>> = vec![None; count];
    let mut classes: Vec<(usize, usize)> = Vec::new();
    let mut index = 1;
    while index < count {
        match take(1)?[0] {
            1 => {
                let len = u2(take(2)?)?;
                utf8[index] = Some(String::from_utf8_lossy(take(len)?).into_owned());
            }
            5 | 6 => {
                take(8)?;
                index += 1;
            }
            7 => classes.push((index, u2(take(2)?)?)),
            8 | 16 | 19 | 20 => {
                take(2)?;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => {
                take(4)?;
            }
            15 => {
                take(3)?;
            }
            _ => return None,
        }
        index += 1;
    }
    take(2)?;
    let this = u2(take(2)?)?;
    let name_index = classes.iter().find(|(at, _)| *at == this)?.1;
    let name = utf8.get(name_index)?.clone()?;

    let mut refs = BTreeSet::new();
    for (_, class) in classes {
        if let Some(Some(value)) = utf8.get(class) {
            if value.starts_with('[') {
                descriptor_refs(value, &mut refs);
            } else {
                refs.insert(value.clone());
            }
        }
    }
    for value in utf8.iter().flatten() {
        if value.contains(';')
            && (value.starts_with('(') || value.starts_with('L') || value.starts_with('['))
        {
            descriptor_refs(value, &mut refs);
        }
    }
    refs.remove(&name);
    Some((name, refs.into_iter().collect()))
}

fn descriptor_refs(descriptor: &str, out: &mut BTreeSet<String>) {
    let bytes = descriptor.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'L' {
            let rest = &descriptor[i + 1..];
            if let Some(end) = rest.find(';') {
                let cut = rest[..end].find('<').unwrap_or(end);
                let name = &rest[..cut];
                if !name.is_empty() && !name.contains(['(', ')', ' ']) {
                    out.insert(name.to_string());
                }
                i += 1 + cut;
                continue;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_manifest_keeps_version_ranges() {
        let meta = parse_fabric(
            r#"{"id":"sodium","version":"0.5.8","provides":["indium-compat"],
                "depends":{"fabricloader":">=0.15","fabric-api":["^0.90","^0.91"]},
                "breaks":{"optifabric":"*"}}"#,
        )
        .unwrap();
        assert_eq!(meta.id, "sodium");
        assert_eq!(meta.provides, vec!["indium-compat"]);
        assert_eq!(meta.depends["fabric-api"].len(), 2);
        assert!(meta.breaks.contains_key("optifabric"));
    }

    #[test]
    fn forge_manifest_maps_dependency_types() {
        let meta = parse_forge(
            r#"
            [[mods]]
            modId="create"
            version="${file.jarVersion}"
            [[dependencies.create]]
            modId="flywheel"
            type="required"
            versionRange="[1.0,2.0)"
            [[dependencies.create]]
            modId="jei"
            mandatory=false
            [[dependencies.create]]
            modId="optifine"
            type="incompatible"
            "#,
            Some("0.5.1"),
        )
        .unwrap();
        assert_eq!(meta.id, "create");
        assert_eq!(meta.version.to_string(), "0.5.1");
        assert!(meta.depends.contains_key("flywheel"));
        assert!(meta.recommends.contains_key("jei"));
        assert!(meta.breaks.contains_key("optifine"));
    }

    #[test]
    fn an_unfilled_jar_version_stays_unknown() {
        let meta = parse_forge(
            r#"
            [[mods]]
            modId="flywheel"
            version="${file.jarVersion}"
            "#,
            None,
        )
        .unwrap();
        assert_eq!(meta.version, Version::Unknown);
    }

    #[test]
    fn legacy_forge_reads_required_mods() {
        let meta = parse_legacy(
            r#"[{"modid":"patcher","version":"1.8.0","requiredMods":["Forge@[11.15,)","oneconfig"]}]"#,
        )
        .unwrap();
        assert!(meta.depends.contains_key("forge"));
        assert!(meta.depends.contains_key("oneconfig"));
        assert!(meta.loose_id, "mcmod.info ids are not used to spot duplicates");
    }

    #[test]
    fn class_files_yield_their_name_and_references() {
        let utf8 = |text: &str| {
            let mut entry = vec![1u8];
            entry.extend((text.len() as u16).to_be_bytes());
            entry.extend(text.as_bytes());
            entry
        };
        let mut class = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 8];
        class.extend(utf8("com/a/Main"));
        class.extend([7, 0, 1]);
        class.extend(utf8("com/b/Lib"));
        class.extend([7, 0, 3]);
        class.extend([5, 0, 0, 0, 0, 0, 0, 0, 1]);
        class.extend(utf8("(Lcom/c/Arg;)V"));
        class.extend([0, 33, 0, 2]);

        let (name, refs) = parse_class(&class).unwrap();
        assert_eq!(name, "com/a/Main");
        assert_eq!(refs, vec!["com/b/Lib".to_string(), "com/c/Arg".to_string()]);
        assert!(parse_class(&class[..20]).is_none());
    }

    #[test]
    fn descriptors_yield_class_names() {
        let mut refs = BTreeSet::new();
        descriptor_refs("(Lcom/a/B;[Lcom/c/D<Lcom/e/F;>;)V", &mut refs);
        assert!(refs.contains("com/a/B"));
        assert!(refs.contains("com/c/D"));
        assert!(refs.contains("com/e/F"));
    }
}
