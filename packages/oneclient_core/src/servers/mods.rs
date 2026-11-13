use std::path::Path;

use serde_json::Value;

/// A jar in the server's mods folder with what its own metadata says about it
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerMod {
    pub file_name: String,
    pub size: u64,
    pub enabled: bool,
    pub name: Option<String>,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub icon: Option<Vec<u8>>,
}

const DISABLED: &str = ".disabled";
const META_FILES: [&str; 4] = [
    "fabric.mod.json",
    "quilt.mod.json",
    "META-INF/neoforge.mods.toml",
    "META-INF/mods.toml",
];

pub async fn list_mods(dir: &Path) -> Vec<ServerMod> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir.join("mods")).await else {
        return out;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.ends_with(".jar");
        if !enabled && !file_name.ends_with(&format!(".jar{DISABLED}")) {
            continue;
        }
        let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
        let mut found = read_meta(&entry.path()).await;
        found.file_name = file_name;
        found.size = size;
        found.enabled = enabled;
        out.push(found);
    }
    out
}

async fn read_meta(jar: &Path) -> ServerMod {
    let Ok(entries) = polyio::read_zip_file_entries(jar, |name| META_FILES.contains(&name)).await
    else {
        return ServerMod::default();
    };
    let Some((file, bytes)) = META_FILES
        .iter()
        .find_map(|wanted| entries.iter().find(|(name, _)| name == wanted))
    else {
        return ServerMod::default();
    };
    let mut meta = parse_meta(file, bytes);
    if let Some(icon_path) = meta.icon_path.take() {
        meta.found.icon = polyio::read_zip_file_entries(jar, |name| name == icon_path)
            .await
            .ok()
            .and_then(|mut e| e.pop())
            .map(|(_, bytes)| bytes);
    }
    meta.found
}

struct Parsed {
    found: ServerMod,
    icon_path: Option<String>,
}

fn parse_meta(file: &str, bytes: &[u8]) -> Parsed {
    if file.ends_with(".toml") {
        return parse_toml(&String::from_utf8_lossy(bytes));
    }
    let json: Value = serde_json::from_slice(bytes).unwrap_or_default();
    let quilt = file == "quilt.mod.json";
    let meta = if quilt {
        json.pointer("/quilt_loader/metadata")
    } else {
        Some(&json)
    };
    let text = |v: Option<&Value>| {
        v.and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };

    let authors = if quilt {
        meta.and_then(|m| m.get("contributors"))
            .and_then(Value::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        json.get("authors")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|a| {
                        a.as_str()
                            .or_else(|| a.get("name")?.as_str())
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    // An icon is either one path or a map of sizes to paths, the largest reads best scaled down
    let icon_path = meta
        .and_then(|m| m.get("icon"))
        .and_then(|icon| match icon {
            Value::String(path) => Some(path.clone()),
            Value::Object(sizes) => sizes
                .iter()
                .max_by_key(|(size, _)| size.parse::<u32>().unwrap_or(0))
                .and_then(|(_, path)| path.as_str().map(str::to_string)),
            _ => None,
        });

    Parsed {
        found: ServerMod {
            name: text(meta.and_then(|m| m.get("name"))),
            version: text(if quilt {
                json.pointer("/quilt_loader/version")
            } else {
                json.get("version")
            }),
            authors,
            description: text(meta.and_then(|m| m.get("description"))),
            ..ServerMod::default()
        },
        icon_path: icon_path.map(|p| p.trim_start_matches('/').to_string()),
    }
}

/// shortcut: reads single-line `key = "value"` pairs of the first mod, multi-line values are skipped
fn parse_toml(text: &str) -> Parsed {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (k, v) = line.split_once('=')?;
            if k.trim() != key {
                return None;
            }
            let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim();
            (!v.is_empty() && !v.contains("${")).then(|| v.to_string())
        })
    };
    Parsed {
        found: ServerMod {
            name: value("displayName"),
            version: value("version"),
            authors: value("authors").map(|a| vec![a]).unwrap_or_default(),
            description: value("description"),
            ..ServerMod::default()
        },
        icon_path: value("logoFile"),
    }
}

/// Renames the jar so the loader skips it, or picks it up again
pub async fn set_mod_enabled(dir: &Path, file_name: &str, enabled: bool) -> std::io::Result<()> {
    let mods = dir.join("mods");
    let base = file_name.trim_end_matches(DISABLED);
    let (from, to) = if enabled {
        (format!("{base}{DISABLED}"), base.to_string())
    } else {
        (base.to_string(), format!("{base}{DISABLED}"))
    };
    tokio::fs::rename(mods.join(from), mods.join(to)).await
}

#[cfg(test)]
mod tests {
    use super::parse_meta;

    #[test]
    fn reads_fabric_and_forge_metadata() {
        let fabric = parse_meta(
            "fabric.mod.json",
            br#"{"id":"lithium","name":"Lithium","version":"0.15.0","authors":["jellysquid3",{"name":"2No2Name"}],"icon":{"16":"a.png","128":"assets/lithium/icon.png"}}"#,
        );
        assert_eq!(fabric.found.name.as_deref(), Some("Lithium"));
        assert_eq!(fabric.found.version.as_deref(), Some("0.15.0"));
        assert_eq!(fabric.found.authors, ["jellysquid3", "2No2Name"]);
        assert_eq!(fabric.icon_path.as_deref(), Some("assets/lithium/icon.png"));

        let forge = parse_meta(
            "META-INF/mods.toml",
            b"modLoader=\"javafml\"\nlogoFile=\"logo.png\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName=\"Just Enough Items\"\nauthors=\"mezz\"\n",
        );
        assert_eq!(forge.found.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(forge.found.version, None);
        assert_eq!(forge.found.authors, ["mezz"]);
        assert_eq!(forge.icon_path.as_deref(), Some("logo.png"));
    }
}
