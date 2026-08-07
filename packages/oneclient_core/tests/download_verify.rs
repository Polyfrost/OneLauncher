use oneclient_core::dev;
use oneclient_core::game::download_to_path;
use oneclient_events::{GroupedProgressSession, TaskCategory};
use polyio::testing::ScratchDir as Scratch;
use polyio::{Sha1Stream, sha1_bytes, sha1_file};

/// Minecraft asset objects are content-addressed and immutable so this
/// URL/hash pair stays valid
const ASSET_SHA1: &str = "af96f55a90eaf11b327f1b5f8834a051027dc506";
const ASSET_URL: &str =
    "https://resources.download.minecraft.net/af/af96f55a90eaf11b327f1b5f8834a051027dc506";
const ASSET_SIZE: u64 = 2063;

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

    // Larger than the hashing buffer so the multi-read path is covered too
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

/// Serves `body` with a 200 declaring `content_length` however long `body` is
/// pass a larger one to imitate a mirror holding the wrong file
async fn serve(body: Vec<u8>, content_length: usize) -> String {
    use tokio::io::AsyncWriteExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let body = body.clone();

            tokio::spawn(async move {
                let mut request = [0u8; 1024];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut request).await;

                let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\n\r\n");
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
                let _ = stream.flush().await;
            });
        }
    });

    format!("http://127.0.0.1:{port}/artifact.jar")
}

/// Answers the first `failures` requests with `status` then serves `ok_body`
/// Returns the URL and a request counter
/// `usize::MAX` never recovers
async fn serve_flaky(
    status: u16,
    reason: &'static str,
    failures: usize,
    error_body: Vec<u8>,
    ok_body: Vec<u8>,
) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::AsyncWriteExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let served = Arc::new(AtomicUsize::new(0));

    tokio::spawn({
        let served = Arc::clone(&served);
        async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (error_body, ok_body) = (error_body.clone(), ok_body.clone());
                let served = Arc::clone(&served);

                tokio::spawn(async move {
                    // Draining the request first or the client sees a reset
                    let mut request = [0u8; 1024];
                    let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut request).await;

                    let seen = served.fetch_add(1, Ordering::SeqCst);
                    let (status, reason, body) = if seen < failures {
                        (status, reason, error_body)
                    } else {
                        (200, "OK", ok_body)
                    };

                    let head = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                    let _ = stream.flush().await;
                });
            }
        }
    });

    (format!("http://127.0.0.1:{port}/artifact.jar"), served)
}

#[tokio::test]
async fn download_to_path_never_writes_an_error_response_to_disk() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-404");
    let dest = dir.join("artifact.jar");

    // What a CDN answers for a missing object well-formed but XML not a jar
    let (url, served) = serve_flaky(
        404,
        "Not Found",
        usize::MAX,
        b"<Error><Code>NoSuchKey</Code></Error>".to_vec(),
        Vec::new(),
    )
    .await;

    let err = download_to_path(
        &services.requester,
        &services.events,
        &progress,
        "artifact",
        TaskCategory::Libraries,
        0,
        &url,
        &dest,
        None,
    )
    .await
    .expect_err("a 404 must not be saved as the file");

    assert!(err.to_string().contains("404"), "{err}");
    assert!(err.to_string().contains("NoSuchKey"), "{err}");
    assert!(!dest.exists(), "an error body must never reach disk");
    // A 404 answers the same way every time retrying it only delays the error
    assert_eq!(served.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn download_to_path_retries_a_server_error_and_recovers() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-500");
    let dest = dir.join("artifact.jar");

    // Unlike a 404 a 500 can succeed on the next attempt
    let (url, served) = serve_flaky(
        500,
        "Internal Server Error",
        1,
        b"boom".to_vec(),
        vec![7u8; 128],
    )
    .await;

    download_to_path(
        &services.requester,
        &services.events,
        &progress,
        "artifact",
        TaskCategory::Libraries,
        128,
        &url,
        &dest,
        None,
    )
    .await
    .expect("a retried server error should recover");

    assert_eq!(std::fs::metadata(&dest).unwrap().len(), 128);
    assert_eq!(served.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn download_to_path_rejects_a_body_shorter_than_the_manifest_size() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-short");
    let dest = dir.join("artifact.jar");

    let url = serve(vec![7u8; 64], 64).await;

    let err = download_to_path(
        &services.requester,
        &services.events,
        &progress,
        "artifact",
        TaskCategory::Libraries,
        4096,
        &url,
        &dest,
        None,
    )
    .await
    .expect_err("a short body should fail without a hash to check");

    assert!(err.to_string().contains("ended after 64 of 4096"), "{err}");
    assert!(!dest.exists(), "truncated download should be removed");
}

#[tokio::test]
async fn download_to_path_accepts_an_unhashed_body_of_the_promised_length() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-exact");
    let dest = dir.join("artifact.jar");

    let url = serve(vec![7u8; 4096], 4096).await;

    download_to_path(
        &services.requester,
        &services.events,
        &progress,
        "artifact",
        TaskCategory::Libraries,
        4096,
        &url,
        &dest,
        None,
    )
    .await
    .expect("a body of the promised length should be kept");

    assert_eq!(std::fs::metadata(&dest).unwrap().len(), 4096);
}

#[tokio::test]
async fn download_to_path_keeps_an_unhashed_body_of_unknown_length() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-unknown");
    let dest = dir.join("artifact.jar");

    let url = serve(vec![7u8; 128], 128).await;

    // Nothing declared a size so there is nothing to check against
    download_to_path(
        &services.requester,
        &services.events,
        &progress,
        "artifact",
        TaskCategory::Libraries,
        0,
        &url,
        &dest,
        None,
    )
    .await
    .expect("an unmeasurable download should still be written");

    assert_eq!(std::fs::metadata(&dest).unwrap().len(), 128);
}

#[tokio::test]
#[ignore = "requires network"]
async fn download_to_path_accepts_matching_hash() {
    let services = dev::ephemeral_services().await.unwrap();
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-ok");
    let dest = dir.join("nested").join("icon.png");

    download_to_path(
        &services.requester,
        &services.events,
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
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-bad");
    let dest = dir.join("icon.png");

    let err = download_to_path(
        &services.requester,
        &services.events,
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
    let progress = GroupedProgressSession::start(&services.events, "test");
    let dir = Scratch::new("download-nohash");
    let dest = dir.join("icon.png");

    download_to_path(
        &services.requester,
        &services.events,
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
