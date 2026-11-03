use oneclient_content::packages::ProviderId;

use crate::i18n::t;

#[derive(Debug, Clone, Copy)]
pub struct EssentialPackage {
    pub provider: ProviderId,
    pub project_id: &'static str,
    pub name: &'static str,
    pub disable_body: fn() -> &'static str,
    pub remove_body: fn() -> &'static str,
}

impl PartialEq for EssentialPackage {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider && self.project_id == other.project_id
    }
}

impl Eq for EssentialPackage {}

pub const ESSENTIAL_PACKAGES: &[EssentialPackage] = &[EssentialPackage {
    provider: ProviderId::Modrinth,
    project_id: "Iw9mZi4a",
    name: "PolyPlus+",
    disable_body: || t("oneclient.essential.polyplus.disable_body"),
    remove_body: || t("oneclient.essential.polyplus.remove_body"),
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
