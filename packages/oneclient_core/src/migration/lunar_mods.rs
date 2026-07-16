use std::path::Path;

use serde_json::{Map, Value, json};

use crate::LauncherResult;

#[derive(Debug, PartialEq)]
pub(super) struct ConfigPatch {
    pub path: &'static str,
    pub values: Map<String, Value>,
}

/// Lunar writes numbers as either JSON numbers or strings ("6847" vs 6329)
/// depending on how the value was last touched, so both have to be accepted.
fn loose_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok().or_else(|| {
            // "0.0"-style values still carry an integer meaning.
            s.trim().parse::<f64>().ok().map(|f| f as i64)
        }),
        _ => None,
    }
}

fn loose_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

struct LunarMod<'a>(&'a Map<String, Value>);

impl<'a> LunarMod<'a> {
    fn enabled(&self) -> bool {
        self.0.get("enabled").and_then(Value::as_bool).unwrap_or(false)
    }

    fn option(&self, key: &str) -> Option<&'a Value> {
        self.0.get("options")?.as_object()?.get(key)
    }
}

fn lunar_mod<'a>(mods: &'a Map<String, Value>, id: &str) -> Option<LunarMod<'a>> {
    Some(LunarMod(mods.get(id)?.as_object()?))
}

pub(super) fn translate(mods: &Map<String, Value>) -> Vec<ConfigPatch> {
    let mut patches = Vec::new();

    if let Some(m) = lunar_mod(mods, "TIME_CHANGER") {
        let mut values = Map::new();
        values.insert("isEnabled".into(), json!(m.enabled()));
        if let Some(time) = loose_i64(m.option("timeChangerTime")) {
            values.insert("time".into(), json!(time));
        }
        patches.push(ConfigPatch {
            path: "config/polytime.json",
            values,
        });
    }

    if let Some(m) = lunar_mod(mods, "WEATHER_CHANGER") {
        patches.push(ConfigPatch {
            path: "config/polyweather.json",
            values: [("isEnabled".to_string(), json!(m.enabled()))]
                .into_iter()
                .collect(),
        });
    }

    if let Some(m) = lunar_mod(mods, "HURT_CAM") {
        let suppressed = m.enabled() && loose_bool(m.option("disableHurtCam")).unwrap_or(false);
        if suppressed {
            patches.push(ConfigPatch {
                path: "config/shaketweaks.json",
                values: [
                    ("disableScreenDamage".to_string(), json!(true)),
                    ("disableHandDamage".to_string(), json!(true)),
                ]
                .into_iter()
                .collect(),
            });
        }
    }

    if let Some(m) = lunar_mod(mods, "ZOOM")
        && let Some(variable) = loose_bool(m.option("variableZoom"))
    {
        patches.push(ConfigPatch {
            path: "config/zoomify.json",
            values: [("scrollZoom".to_string(), json!(variable))]
                .into_iter()
                .collect(),
        });
    }

    patches
}

#[tracing::instrument(level = "debug", skip(values))]
async fn merge_into(path: &Path, values: &Map<String, Value>) -> LauncherResult<()> {
    let mut existing: Map<String, Value> = match polyio::read(path).await {
        Ok(raw) => serde_json::from_slice(&raw).unwrap_or_else(|err| {
            tracing::warn!(path = %path.display(), error = %err, "unparsable mod config; rewriting");
            Map::new()
        }),
        Err(_) => Map::new(),
    };

    for (key, value) in values {
        existing.insert(key.clone(), value.clone());
    }

    if let Some(parent) = path.parent() {
        polyio::create_dir_all(parent).await?;
    }

    let rendered = serde_json::to_vec_pretty(&Value::Object(existing))?;
    polyio::write(path, rendered).await?;

    Ok(())
}

#[tracing::instrument(level = "debug", skip(mods))]
pub(super) async fn apply(mods: &Map<String, Value>, dest: &Path) -> LauncherResult<()> {
    let patches = translate(mods);

    for patch in &patches {
        merge_into(&dest.join(patch.path), &patch.values).await?;
    }

    tracing::info!(configs = patches.len(), "lunar migration: wrote mod settings");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mods(raw: serde_json::Value) -> Map<String, Value> {
        raw.as_object().expect("object").clone()
    }

    fn patch<'a>(patches: &'a [ConfigPatch], path: &str) -> Option<&'a ConfigPatch> {
        patches.iter().find(|p| p.path == path)
    }

    #[test]
    fn time_changer_carries_enabled_state_and_tick() {
        // Lunar writes this tick as a string in some profiles.
        let patches = translate(&mods(json!({
            "TIME_CHANGER": { "enabled": true, "options": { "timeChangerTime": "6847" } }
        })));

        let p = patch(&patches, "config/polytime.json").expect("polytime");
        assert_eq!(p.values["isEnabled"], json!(true));
        assert_eq!(p.values["time"], json!(6847));
    }

    #[test]
    fn numeric_and_string_ticks_agree() {
        let as_number = translate(&mods(json!({
            "TIME_CHANGER": { "options": { "timeChangerTime": 6329 } }
        })));
        let as_string = translate(&mods(json!({
            "TIME_CHANGER": { "options": { "timeChangerTime": "6329" } }
        })));

        assert_eq!(
            patch(&as_number, "config/polytime.json").unwrap().values["time"],
            patch(&as_string, "config/polytime.json").unwrap().values["time"],
        );
    }

    #[test]
    fn a_mod_lunar_never_enabled_is_carried_over_as_disabled() {
        // Lunar omits `enabled` rather than writing false.
        let patches = translate(&mods(json!({
            "TIME_CHANGER": { "options": { "timeChangerTime": "0" } }
        })));

        assert_eq!(
            patch(&patches, "config/polytime.json").unwrap().values["isEnabled"],
            json!(false)
        );
    }

    #[test]
    fn hurt_cam_only_suppresses_shake_when_lunar_did() {
        let suppressing = translate(&mods(json!({
            "HURT_CAM": { "enabled": true, "options": { "disableHurtCam": true } }
        })));
        let p = patch(&suppressing, "config/shaketweaks.json").expect("shaketweaks");
        assert_eq!(p.values["disableScreenDamage"], json!(true));
        assert_eq!(p.values["disableHandDamage"], json!(true));

        // Enabled but not actually disabling the cam -> leave shaketweaks alone.
        let passive = translate(&mods(json!({
            "HURT_CAM": { "enabled": true, "options": { "disableHurtCam": false } }
        })));
        assert!(patch(&passive, "config/shaketweaks.json").is_none());
    }

    #[test]
    fn zoom_maps_variable_zoom_to_scroll_zoom() {
        let patches = translate(&mods(json!({
            "ZOOM": { "options": { "variableZoom": true } }
        })));

        assert_eq!(
            patch(&patches, "config/zoomify.json").unwrap().values["scrollZoom"],
            json!(true)
        );
    }

    #[test]
    fn a_mod_with_no_equivalent_produces_nothing() {
        let patches = translate(&mods(json!({
            "TIER_TAGGER": { "enabled": true },
            "SKYBLOCK": { "enabled": true },
        })));

        assert!(patches.is_empty());
    }

    #[tokio::test]
    async fn merging_keeps_keys_the_patch_says_nothing_about() {
        let dir = std::env::temp_dir().join(format!("oneclient-lunar-mods-{}", uuid::Uuid::new_v4()));
        let path = dir.join("config/zoomify.json");
        polyio::create_dir_all(path.parent().unwrap()).await.unwrap();
        polyio::write(&path, br#"{"initialZoom":4,"scrollZoom":false}"#.to_vec())
            .await
            .unwrap();

        let values = [("scrollZoom".to_string(), json!(true))]
            .into_iter()
            .collect();
        merge_into(&path, &values).await.unwrap();

        let raw = polyio::read(&path).await.unwrap();
        let out: Map<String, Value> = serde_json::from_slice(&raw).unwrap();

        assert_eq!(out["scrollZoom"], json!(true));
        // Untouched key survives.
        assert_eq!(out["initialZoom"], json!(4));
    }
}
