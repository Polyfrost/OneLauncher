//! Rich embeds (videos) found in markdown.
//!
//! Nothing here runs provider code. An embed is only ever recognised from its url, rendered as a
//! native card, and opened in the user's browser through the usual link confirmation.

use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    YouTube,
    Vimeo,
    Twitch,
    Streamable,
    Dailymotion,
    Bilibili,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::YouTube => "YouTube",
            Self::Vimeo => "Vimeo",
            Self::Twitch => "Twitch",
            Self::Streamable => "Streamable",
            Self::Dailymotion => "Dailymotion",
            Self::Bilibili => "Bilibili",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Embed {
    pub provider: Provider,
    /// The provider's own page for the content, what the card opens
    pub open_url: String,
    /// A still frame to show on the card, only for providers that publish one at a stable url
    pub thumbnail: Option<String>,
    /// The author's title, absent when it was missing or only the provider's generic one
    pub title: Option<String>,
}

impl Embed {
    /// Recognises a provider url.
    ///
    /// Embed-form urls (`/embed/..`, `player.vimeo.com/..`) are always recognised. Page-form urls
    /// (`watch?v=..`, `youtu.be/..`) only when `allow_page` is set, so an ordinary inline link to a
    /// video stays a link.
    pub fn from_url(raw: &str, title: Option<&str>, allow_page: bool) -> Option<Self> {
        let url = Url::parse(raw.trim()).ok()?;
        if !matches!(url.scheme(), "http" | "https") {
            return None;
        }

        let host = url.host_str()?.to_ascii_lowercase();
        let host = host.strip_prefix("www.").unwrap_or(&host);
        let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
        let query = |key: &str| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.into_owned())
        };

        let (provider, mut open_url, thumbnail) = match host {
            "youtube.com" | "m.youtube.com" | "youtube-nocookie.com" => match segments.as_slice() {
                ["embed", "videoseries"] => {
                    let list = query("list").filter(|id| is_id(id, 60))?;
                    (
                        Provider::YouTube,
                        format!("https://www.youtube.com/playlist?list={list}"),
                        None,
                    )
                }
                ["embed", id] => youtube(id)?,
                ["watch"] if allow_page => youtube(&query("v")?)?,
                ["shorts" | "live", id] if allow_page => youtube(id)?,
                _ => return None,
            },
            "youtu.be" if allow_page => youtube(segments.first()?)?,
            "player.vimeo.com" => match segments.as_slice() {
                ["video", id] if is_digits(id) => vimeo(id, query("h").as_deref()),
                _ => return None,
            },
            "vimeo.com" if allow_page => match segments.as_slice() {
                [id, hash] if is_digits(id) && is_id(hash, 64) => vimeo(id, Some(hash)),
                [.., id] if is_digits(id) => vimeo(id, query("h").as_deref()),
                _ => return None,
            },
            "player.twitch.tv" => {
                if let Some(video) = query("video") {
                    let id = video.strip_prefix('v').unwrap_or(&video);
                    twitch_video(id)?
                } else {
                    let channel = query("channel").filter(|c| is_id(c, 25))?;
                    (
                        Provider::Twitch,
                        format!("https://www.twitch.tv/{channel}"),
                        None,
                    )
                }
            }
            "twitch.tv" if allow_page => match segments.as_slice() {
                ["videos", id] => twitch_video(id)?,
                _ => return None,
            },
            "streamable.com" => match segments.as_slice() {
                ["e", id] => streamable(id)?,
                [id] if allow_page => streamable(id)?,
                _ => return None,
            },
            "dailymotion.com" => match segments.as_slice() {
                ["embed", "video", id] => dailymotion(id)?,
                ["video", id] if allow_page => dailymotion(id)?,
                _ => return None,
            },
            "player.bilibili.com" => bilibili(&query("bvid")?)?,
            "bilibili.com" if allow_page => match segments.as_slice() {
                ["video", id] => bilibili(id)?,
                _ => return None,
            },
            _ => return None,
        };

        if provider == Provider::YouTube
            && thumbnail.is_some()
            && let Some(start) = query("start")
                .filter(|value| is_digits(value))
                .or_else(|| query("t").filter(|value| is_youtube_time(value)))
        {
            open_url.push_str("&t=");
            open_url.push_str(&start);
        }

        if provider == Provider::Bilibili
            && let Some(part) = query("p")
                .filter(|value| is_digits(value))
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|part| *part > 0)
        {
            open_url.push_str("?p=");
            open_url.push_str(&part.to_string());
        }

        Some(Self {
            provider,
            open_url,
            thumbnail,
            title: title.and_then(usable_title),
        })
    }
}

fn youtube(id: &str) -> Option<(Provider, String, Option<String>)> {
    (id.len() == 11 && is_id(id, 11)).then(|| {
        (
            Provider::YouTube,
            format!("https://www.youtube.com/watch?v={id}"),
            Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")),
        )
    })
}

fn vimeo(id: &str, hash: Option<&str>) -> (Provider, String, Option<String>) {
    let mut open_url = format!("https://vimeo.com/{id}");
    if let Some(hash) = hash.filter(|hash| is_id(hash, 64)) {
        open_url.push('/');
        open_url.push_str(hash);
    }
    (Provider::Vimeo, open_url, None)
}

fn is_youtube_time(mut time: &str) -> bool {
    if is_digits(time) {
        return true;
    }
    if time.is_empty() || time.len() > 20 {
        return false;
    }

    for unit in ['h', 'm', 's'] {
        if let Some((amount, rest)) = time.split_once(unit) {
            if !is_digits(amount) {
                return false;
            }
            time = rest;
        }
    }
    time.is_empty()
}

fn twitch_video(id: &str) -> Option<(Provider, String, Option<String>)> {
    is_digits(id).then(|| {
        (
            Provider::Twitch,
            format!("https://www.twitch.tv/videos/{id}"),
            None,
        )
    })
}

fn streamable(id: &str) -> Option<(Provider, String, Option<String>)> {
    is_id(id, 12).then(|| {
        (
            Provider::Streamable,
            format!("https://streamable.com/{id}"),
            None,
        )
    })
}

fn dailymotion(id: &str) -> Option<(Provider, String, Option<String>)> {
    is_id(id, 20).then(|| {
        (
            Provider::Dailymotion,
            format!("https://www.dailymotion.com/video/{id}"),
            None,
        )
    })
}

fn bilibili(id: &str) -> Option<(Provider, String, Option<String>)> {
    (id.starts_with("BV") && is_id(id, 20)).then(|| {
        (
            Provider::Bilibili,
            format!("https://www.bilibili.com/video/{id}"),
            None,
        )
    })
}

/// A provider id is a short run of url-safe characters, never anything that could change the
/// shape of the url it is spliced into.
fn is_id(id: &str, max_len: usize) -> bool {
    !id.is_empty()
        && id.len() <= max_len
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_digits(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit())
}

/// Providers fill the iframe title with a boilerplate such as "YouTube video player", which says
/// nothing the card does not already.
fn usable_title(title: &str) -> Option<String> {
    let title = title.trim();
    let generic = title.to_ascii_lowercase().ends_with("video player");

    (!title.is_empty() && !generic && !title.starts_with("http")).then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embed(url: &str) -> Embed {
        Embed::from_url(url, None, true).unwrap_or_else(|| panic!("{url} should be an embed"))
    }

    #[test]
    fn recognises_the_wwaypoints_youtube_nocookie_embed() {
        let found = Embed::from_url(
            "https://www.youtube-nocookie.com/embed/WaRLLta2Edg",
            Some("YouTube video player"),
            false,
        )
        .expect("embed form is recognised without allow_page");

        assert_eq!(found.provider, Provider::YouTube);
        assert_eq!(
            found.open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg"
        );
        assert_eq!(
            found.thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/WaRLLta2Edg/mqdefault.jpg")
        );
        assert_eq!(found.title, None, "the generic player title is dropped");
    }

    #[test]
    fn keeps_an_authors_own_title() {
        let found = Embed::from_url(
            "https://www.youtube.com/embed/WaRLLta2Edg",
            Some("Watch Sheet's Showcase"),
            false,
        )
        .unwrap();
        assert_eq!(found.title.as_deref(), Some("Watch Sheet's Showcase"));
    }

    #[test]
    fn page_urls_need_allow_page() {
        for url in [
            "https://www.youtube.com/watch?v=WaRLLta2Edg",
            "https://youtu.be/WaRLLta2Edg",
            "https://www.youtube.com/shorts/WaRLLta2Edg",
            "https://vimeo.com/76979871",
            "https://streamable.com/abc123",
        ] {
            assert!(Embed::from_url(url, None, false).is_none(), "{url}");
            assert!(Embed::from_url(url, None, true).is_some(), "{url}");
        }
    }

    #[test]
    fn recognises_other_providers() {
        assert_eq!(
            embed("https://player.vimeo.com/video/76979871").open_url,
            "https://vimeo.com/76979871"
        );
        assert_eq!(
            embed("https://player.twitch.tv/?video=v123456&parent=example.com").open_url,
            "https://www.twitch.tv/videos/123456"
        );
        assert_eq!(
            embed("https://streamable.com/e/abc123").open_url,
            "https://streamable.com/abc123"
        );
        assert_eq!(
            embed("https://www.dailymotion.com/embed/video/x7tgad0").open_url,
            "https://www.dailymotion.com/video/x7tgad0"
        );
        assert_eq!(
            embed("https://player.bilibili.com/player.html?bvid=BV1xx411c7mD").provider,
            Provider::Bilibili
        );
        assert_eq!(
            embed("https://www.youtube.com/embed/videoseries?list=PLabc-123").open_url,
            "https://www.youtube.com/playlist?list=PLabc-123"
        );
    }

    #[test]
    fn preserves_youtube_seek_without_forwarding_other_parameters() {
        assert_eq!(
            embed("https://www.youtube-nocookie.com/embed/WaRLLta2Edg?start=120&autoplay=1")
                .open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg&t=120"
        );
        assert_eq!(
            embed("https://youtu.be/WaRLLta2Edg?t=1h2m3s").open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg&t=1h2m3s"
        );
        assert_eq!(
            embed("https://youtu.be/WaRLLta2Edg?t=12%26autoplay%3d1").open_url,
            "https://www.youtube.com/watch?v=WaRLLta2Edg"
        );
    }

    #[test]
    fn preserves_only_valid_bilibili_parts() {
        assert_eq!(
            embed("https://player.bilibili.com/player.html?bvid=BV1xx411c7mD&p=2&autoplay=1")
                .open_url,
            "https://www.bilibili.com/video/BV1xx411c7mD?p=2"
        );
        assert_eq!(
            embed("https://www.bilibili.com/video/BV1xx411c7mD?p=0").open_url,
            "https://www.bilibili.com/video/BV1xx411c7mD"
        );
        assert_eq!(
            embed("https://www.bilibili.com/video/BV1xx411c7mD?p=2%26autoplay%3D1").open_url,
            "https://www.bilibili.com/video/BV1xx411c7mD"
        );
    }

    #[test]
    fn preserves_vimeo_unlisted_access_hash() {
        let unlisted = "https://vimeo.com/76979871/87ec5e9993";
        assert_eq!(
            embed("https://player.vimeo.com/video/76979871?h=87ec5e9993&autoplay=1").open_url,
            unlisted
        );
        assert_eq!(embed(unlisted).open_url, unlisted);
        assert_eq!(
            embed("https://player.vimeo.com/video/76979871?h=abc%2Fxyz").open_url,
            "https://vimeo.com/76979871"
        );
    }

    #[test]
    fn rejects_unknown_hosts_schemes_and_malformed_ids() {
        for url in [
            "https://evil.example/embed/WaRLLta2Edg",
            "https://youtube.com.evil.example/embed/WaRLLta2Edg",
            "https://www.youtube.com/embed/short",
            "https://www.youtube.com/embed/WaRLLta2Edg/extra",
            "https://www.youtube.com/embed/Wa/../x",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,<script>alert(1)</script>",
            "not a url",
            "https://player.vimeo.com/video/notanumber",
        ] {
            assert!(Embed::from_url(url, None, true).is_none(), "{url}");
        }
    }
}
