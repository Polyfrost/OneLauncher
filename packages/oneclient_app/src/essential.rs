use oneclient_content::packages::ProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EssentialPackage {
    pub provider: ProviderId,
    pub project_id: &'static str,
    pub name: &'static str,
    pub disable_body: &'static str,
    pub remove_body: &'static str,
}

pub const ESSENTIAL_PACKAGES: &[EssentialPackage] = &[EssentialPackage {
    provider: ProviderId::Modrinth,
    project_id: "Iw9mZi4a",
    name: "PolyPlus+",
    disable_body: "PolyPlus+ powers your Poly+ cosmetics and playtime tracking. Turning it off disables them in game, and it stays off through future bundle updates until you turn it back on.",
    remove_body: "PolyPlus+ powers your Poly+ cosmetics and playtime tracking. Removing it disables them in game and deletes the file from this cluster, so you would have to install it again from the Browser.",
}];

pub fn lookup(provider: ProviderId, package_id: &str) -> Option<&'static EssentialPackage> {
    ESSENTIAL_PACKAGES
        .iter()
        .find(|package| package.provider == provider && package.project_id == package_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polyplus_is_essential_on_modrinth_only() {
        assert_eq!(
            lookup(ProviderId::Modrinth, "Iw9mZi4a").map(|p| p.name),
            Some("PolyPlus+")
        );
        assert!(lookup(ProviderId::CurseForge, "Iw9mZi4a").is_none());
        assert!(lookup(ProviderId::Local, "Iw9mZi4a").is_none());
    }

    #[test]
    fn ordinary_packages_are_not_guarded() {
        assert!(lookup(ProviderId::Modrinth, "AibBIVmj").is_none());
    }
}
