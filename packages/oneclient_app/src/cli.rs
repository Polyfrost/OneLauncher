#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cli {
    pub launch: Option<String>,
    pub share: Option<String>,
}

impl Cli {
    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut cli = Self::default();
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            if let Some(value) = arg.strip_prefix("--launch=") {
                cli.launch = non_empty(value.to_string());
            } else if arg == "--launch" {
                cli.launch = args.next().and_then(non_empty);
            } else if let Some(value) = arg.strip_prefix("--share=") {
                cli.share = share_code(value);
            } else if arg == "--share" {
                cli.share = args.next().as_deref().and_then(share_code);
            } else if let Some(folder) = crate::protocol::parse_launch_url(&arg) {
                cli.launch = Some(folder);
            } else if let Some(code) = crate::protocol::parse_share_url(&arg) {
                cli.share = Some(code);
            }
        }

        cli
    }
}

fn share_code(value: &str) -> Option<String> {
    oneclient_core::sharing::ShareCode::parse(value).map(|code| code.as_str().to_string())
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[must_use]
pub fn parse() -> Cli {
    Cli::from_args(std::env::args().skip(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::from_args(args.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn no_arguments_is_an_ordinary_start() {
        assert_eq!(parse(&[]).launch, None);
    }

    #[test]
    fn both_spellings_carry_the_folder() {
        assert_eq!(
            parse(&["--launch", "fabric-1-20"]).launch.as_deref(),
            Some("fabric-1-20")
        );
        assert_eq!(
            parse(&["--launch=fabric-1-20"]).launch.as_deref(),
            Some("fabric-1-20")
        );
    }

    #[test]
    fn a_blank_folder_is_no_request_at_all() {
        assert_eq!(parse(&["--launch", "   "]).launch, None);
        assert_eq!(parse(&["--launch="]).launch, None);
        assert_eq!(parse(&["--launch"]).launch, None);
    }

    #[test]
    fn spaces_survive_the_round_trip() {
        assert_eq!(
            parse(&["--launch", "My Pack (1.8.9)"]).launch.as_deref(),
            Some("My Pack (1.8.9)"),
        );
    }

    #[test]
    fn unknown_flags_are_ignored_not_fatal() {
        assert_eq!(
            parse(&["--verbose", "--launch", "x", "leftover"])
                .launch
                .as_deref(),
            Some("x")
        );
    }

    #[test]
    fn a_launch_url_is_accepted_as_a_bare_argument() {
        let url = crate::protocol::launch_url("26.1.2 Fabric");
        assert_eq!(parse(&[&url]).launch.as_deref(), Some("26.1.2 Fabric"));
    }

    #[test]
    fn a_share_link_or_flag_carries_the_code() {
        let url = format!("{}://share/abcd-efgh", crate::protocol::SCHEME);
        assert_eq!(parse(&[&url]).share.as_deref(), Some("ABCDEFGH"));
        assert_eq!(
            parse(&["--share", "ABCD-EFGH"]).share.as_deref(),
            Some("ABCDEFGH")
        );
        assert_eq!(
            parse(&["--share=abcdefgh"]).share.as_deref(),
            Some("ABCDEFGH")
        );
        assert_eq!(parse(&["--share", "nope"]).share, None);
        assert_eq!(parse(&[&url]).launch, None);
    }

    #[test]
    fn a_url_that_is_not_ours_is_not_a_launch() {
        assert_eq!(parse(&["https://polyfrost.org"]).launch, None);
    }
}
