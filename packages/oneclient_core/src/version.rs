
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedMcVersion {
    pub major: u32,
    pub minor: Option<u32>,
    pub patch: Option<u32>,
}

impl ParsedMcVersion {
    #[must_use]
    pub fn key(&self) -> Option<VersionKey> {
        Some((self.minor?, self.patch))
    }
}

pub type VersionKey = (u32, Option<u32>);

pub fn parse_mc_version(version: &str) -> Option<ParsedMcVersion> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.first() != Some(&"1") {
        if parts.len() < 2 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = match parts.get(2) {
            Some(p) => Some(p.parse().ok()?),
            None => None,
        };
        return Some(ParsedMcVersion {
            major,
            minor: Some(minor),
            patch,
        });
    }

    if parts.len() <= 1 {
        return None;
    }

    let major = parts[1].parse().ok()?;
    let minor = parts.get(2).and_then(|p| p.parse().ok());

    Some(ParsedMcVersion {
        major,
        minor,
        patch: None,
    })
}

#[must_use]
pub fn format_mc_version(major: u32, minor: u32, patch: Option<u32>) -> String {
    let base = if major >= 26 {
        format!("{major}.{minor}")
    } else {
        format!("1.{major}.{minor}")
    };

    match patch {
        Some(patch) => format!("{base}.{patch}"),
        None => base,
    }
}

#[must_use]
pub fn normalize_mc_version_input(version: &str) -> String {
    version.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_modern_versions() {
        let v = parse_mc_version("26.1").unwrap();
        assert_eq!(v.major, 26);
        assert_eq!(v.minor, Some(1));
        assert_eq!(v.patch, None);

        let v = parse_mc_version("26.2").unwrap();
        assert_eq!(v.major, 26);
        assert_eq!(v.minor, Some(2));
        assert_eq!(v.patch, None);
    }

    #[test]
    fn parse_modern_patch_versions() {
        let v = parse_mc_version("26.1.2").unwrap();
        assert_eq!(v.major, 26);
        assert_eq!(v.minor, Some(1));
        assert_eq!(v.patch, Some(2));
    }

    #[test]
    fn parse_legacy_versions() {
        let v = parse_mc_version("1.21.5").unwrap();
        assert_eq!(v.major, 21);
        assert_eq!(v.minor, Some(5));
        assert_eq!(v.patch, None);
    }

    #[test]
    fn reject_garbage_patch_component() {
        assert!(parse_mc_version("26.1.x").is_none());
    }

    #[test]
    fn format_versions() {
        assert_eq!(format_mc_version(26, 1, None), "26.1");
        assert_eq!(format_mc_version(26, 1, Some(2)), "26.1.2");
        assert_eq!(format_mc_version(21, 5, None), "1.21.5");
    }

    #[test]
    fn format_round_trips_through_parse() {
        for (major, minor, patch) in [(26, 1, None), (26, 1, Some(2)), (21, 5, None)] {
            let text = format_mc_version(major, minor, patch);
            let parsed = parse_mc_version(&text).unwrap();
            assert_eq!(parsed.major, major);
            assert_eq!(parsed.minor, Some(minor));
            assert_eq!(parsed.patch, patch);
        }
    }

    #[test]
    fn patch_sorts_after_bare_minor() {
        let bare = parse_mc_version("26.1").unwrap().key().unwrap();
        let patched = parse_mc_version("26.1.2").unwrap().key().unwrap();
        assert!(bare < patched);
    }
}
