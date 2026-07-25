use oneclient_core::crypto::{Sha1Stream, sha1_bytes, sha1_file};
use oneclient_core::dev;
use oneclient_core::game::download_to_path;
use oneclient_core::notification::{GroupedProgressSession, TaskCategory};

/// A Minecraft asset object. Objects are content-addressed and immutable, so
/// this URL/hash pair stays valid.
const ASSET_SHA1: &str = "af96f55a90eaf11b327f1b5f8834a051027dc506";
const ASSET_URL: &str =
    "https://resources.download.minecraft.net/af/af96f55a90eaf11b327f1b5f8834a051027dc506";
const ASSET_SIZE: u64 = 2063;

/// Scratch directory unique to one test, removed by [`Scratch`]'s drop.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("oneclient-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn sha1_stream_matches_one_shot_hash() {
    let data: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();

    let mut hasher = Sha1Stream::new();
    for chunk in data.chunks(4096) {
        hasher.update(chunk);
    }

    assert_eq!(hasher.finish(), sha1_bytes(&data));
}

#[test]
fn sha1_stream_of_empty_input_matches() {
    assert_eq!(Sha1Stream::new().finish(), sha1_bytes(&[]));
}

#[tokio::test]
async fn sha1_file_matches_sha1_bytes() {
    let dir = Scratch::new("sha1-file");
    let path = dir.join("payload.bin");

    // Larger than the hashing buffer, so the multi-read path is covered too.
    let data: Vec<u8> = (0..600_000u32).map(|i| (i % 253) as u8).collect();
    std::fs::write(&path, &data).unwrap();

    assert_eq!(sha1_file(&path).await.unwrap(), sha1_bytes(&data));
}

#[tokio::test]
async fn sha1_file_handles_empty_file() {
    let dir = Scratch::new("sha1-empty");
    let path = dir.join("empty.bin");
    std::fs::write(&path, []).unwrap();

    assert_eq!(sha1_file(&path).await.unwrap(), sha1_bytes(&[]));
}

#[tokio::test]
#[ignore = "requires network"]
async fn download_to_path_accepts_matching_hash() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.notifier, "test");
    let dir = Scratch::new("download-ok");
    let dest = dir.join("nested").join("icon.png");

    download_to_path(
        &services.requester,
        &services.notifier,
        &progress,
        "icon",
        TaskCategory::Assets,
        ASSET_SIZE,
        ASSET_URL,
        &dest,
        Some(ASSET_SHA1),
    )
    .await
    .expect("download should verify");

    assert_eq!(std::fs::metadata(&dest).unwrap().len(), ASSET_SIZE);
    assert_eq!(sha1_file(&dest).await.unwrap(), ASSET_SHA1);
}

#[tokio::test]
#[ignore = "requires network"]
async fn download_to_path_rejects_mismatched_hash_and_removes_file() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.notifier, "test");
    let dir = Scratch::new("download-bad");
    let dest = dir.join("icon.png");

    let err = download_to_path(
        &services.requester,
        &services.notifier,
        &progress,
        "icon",
        TaskCategory::Assets,
        ASSET_SIZE,
        ASSET_URL,
        &dest,
        Some("0000000000000000000000000000000000000000"),
    )
    .await
    .expect_err("mismatched hash should fail");

    assert!(err.to_string().contains("0000000000000000"), "{err}");
    assert!(!dest.exists(), "corrupt download should be removed");
}

#[tokio::test]
#[ignore = "requires network"]
async fn download_to_path_without_expected_hash_still_writes() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.notifier, "test");
    let dir = Scratch::new("download-nohash");
    let dest = dir.join("icon.png");

    download_to_path(
        &services.requester,
        &services.notifier,
        &progress,
        "icon",
        TaskCategory::Assets,
        0,
        ASSET_URL,
        &dest,
        None,
    )
    .await
    .expect("download should succeed");

    assert_eq!(sha1_file(&dest).await.unwrap(), ASSET_SHA1);
}
