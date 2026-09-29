use crate::manager::ClusterManager;

pub const MAX_NAME_CHARS: usize = 20;
pub const MAX_TAG_CHARS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameProblem {
    Empty,
    TooLong,
    ForbiddenCharacters,
    NoUsableCharacters,
}

impl NameProblem {
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "Enter a name.",
            Self::TooLong => "Use 20 characters or fewer.",
            Self::ForbiddenCharacters => "Use only letters A-Z, digits, spaces and _ - . ( )",
            Self::NoUsableCharacters => "Include at least one letter (A-Z) or digit.",
        }
    }
}

impl std::fmt::Display for NameProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

#[must_use]
pub fn is_allowed_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ' ' | '.' | '(' | ')')
}

pub fn validate_instance_name(name: &str) -> Result<(), NameProblem> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameProblem::Empty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(NameProblem::TooLong);
    }
    if !name.chars().all(is_allowed_name_char) {
        return Err(NameProblem::ForbiddenCharacters);
    }
    if ClusterManager::sanitize_name(name).is_empty() {
        return Err(NameProblem::NoUsableCharacters);
    }
    Ok(())
}

pub fn validate_tag(tag: &str) -> Result<(), NameProblem> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err(NameProblem::Empty);
    }
    if tag.chars().count() > MAX_TAG_CHARS {
        return Err(NameProblem::TooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names() {
        assert_eq!(validate_instance_name("My Survival"), Ok(()));
        assert_eq!(validate_instance_name("Modpack (v2.1)_x-y"), Ok(()));
        assert_eq!(validate_instance_name("   "), Err(NameProblem::Empty));
        assert_eq!(
            validate_instance_name("Świat"),
            Err(NameProblem::ForbiddenCharacters)
        );
        assert_eq!(
            validate_instance_name("世界"),
            Err(NameProblem::ForbiddenCharacters)
        );
        assert_eq!(
            validate_instance_name("a/b"),
            Err(NameProblem::ForbiddenCharacters)
        );
        assert_eq!(
            validate_instance_name("my:world"),
            Err(NameProblem::ForbiddenCharacters)
        );
        assert_eq!(
            validate_instance_name("..."),
            Err(NameProblem::NoUsableCharacters)
        );
        assert_eq!(
            validate_instance_name("abcdefghijklmnopqrstu"),
            Err(NameProblem::TooLong)
        );
        assert_eq!(validate_instance_name("abcdefghijklmnopqrst"), Ok(()));
    }

    #[test]
    fn tags() {
        assert_eq!(validate_tag("pvp"), Ok(()));
        assert_eq!(validate_tag(""), Err(NameProblem::Empty));
        assert_eq!(
            validate_tag("abcdefghijklmnopqrstu"),
            Err(NameProblem::TooLong)
        );
    }
}
