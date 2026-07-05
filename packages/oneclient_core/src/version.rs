
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedMcVersion {
    pub major: u32,
    pub minor: Option<u32>,
}

pub fn parse_mc_version(version: &str) -> Option<ParsedMcVersion> {
    let parts: Vec<&str> = version.split('.').collect();

    if parts.first() != Some(&"1") {
        if parts.len() < 2 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        return Some(ParsedMcVersion {
            major,
            minor: Some(minor),
        });
    }

    if parts.len() <= 1 {
        return None;
    }

    let major = parts[1].parse().ok()?;
    let minor = parts
        .get(2)
        .and_then(|p| p.parse().ok());

    Some(ParsedMcVersion { major, minor })
}

#[must_use]
pub fn format_mc_version(major: u32, minor: u32) -> String {
    if major >= 26 {
        format!("{major}.{minor}")
    } else {
        format!("1.{major}.{minor}")
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

        let v = parse_mc_version("26.2").unwrap();
        assert_eq!(v.major, 26);
        assert_eq!(v.minor, Some(2));
    }

    #[test]
    fn parse_legacy_versions() {
        let v = parse_mc_version("1.21.5").unwrap();
        assert_eq!(v.major, 21);
        assert_eq!(v.minor, Some(5));
    }

    #[test]
    fn format_versions() {
        assert_eq!(format_mc_version(26, 1), "26.1");
        assert_eq!(format_mc_version(21, 5), "1.21.5");
    }
}
