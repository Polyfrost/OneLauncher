use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use oneclient_events::{EventBus, GroupedProgressChild, TaskPhase};
use polyio::{Checksum, ChecksumStream, normalize_hash};
use reqwest::header::{ACCEPT_RANGES, CONTENT_RANGE, RANGE};
use reqwest::{Method, StatusCode};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::error::RequestError;
use crate::request::{HttpRequest, RequestOptions};
use crate::response::{ResponseExt, ResponseNotifyOptions, ResponseOptions};
use crate::service::RequestClient;

/// [`RequestClient::send`] only retries failures before the response headers
/// land this covers failures mid-body which would otherwise silently leave an
/// install incomplete
const MAX_DOWNLOAD_ATTEMPTS: u32 = 3;

const DOWNLOAD_TIMEOUT: Duration = Duration::from_hours(1);

const SINGLE_STREAM_TIMEOUT: Duration = if cfg!(test) {
    Duration::from_secs(3)
} else {
    Duration::from_mins(10)
};

const RANGED_MIN_SIZE: u64 = 16 * 1024 * 1024;
const RANGED_PARTS: u64 = 4;
const RANGED_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

const RESUMABLE_MIN_SIZE: u64 = 1024 * 1024;

const SLOW_WINDOW: Duration = if cfg!(test) {
    Duration::from_millis(300)
} else {
    Duration::from_secs(10)
};
const SLOW_FLOOR_BYTES_PER_SEC: u64 = 64 * 1024;
const MAX_SLOW_RECONNECTS: u32 = 3;

fn file_request(url: &str, range: Option<(u64, u64)>) -> Result<HttpRequest, RequestError> {
    let mut request = reqwest::Request::new(Method::GET, url.parse()?);
    if let Some((start, end)) = range {
        request
            .headers_mut()
            .insert(RANGE, format!("bytes={start}-{}", end - 1).parse()?);
    }
    *request.timeout_mut() = Some(DOWNLOAD_TIMEOUT);
    Ok(HttpRequest {
        request,
        options: RequestOptions::default().own_connection(true),
    })
}

/// Only errors meaning the bytes arrived wrong are retried a 404 bad URL or
/// full disk fails identically every time
fn worth_retrying(err: &RequestError) -> bool {
    matches!(
        err,
        RequestError::ReqwestError(_)
            | RequestError::HashMismatch { .. }
            | RequestError::IncompleteBody { .. }
    )
}

fn retry_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(300 * u64::from(attempt))
}

/// Returns `false` when the file is missing or unreadable
pub async fn matches_on_disk(path: &Path, expected_sha1: &str) -> bool {
    if !path.is_file() {
        return false;
    }

    match polyio::sha1_file(path).await {
        Ok(actual) => normalize_hash(&actual) == normalize_hash(expected_sha1),
        Err(err) => {
            tracing::debug!("could not hash {}: {err}", path.display());
            false
        }
    }
}

/// `dest` is only created once the hash matches so a failed attempt leaves
/// nothing behind for a later `exists()` check to trip over
#[tracing::instrument(level = "debug", skip(client, events, notify), fields(%url))]
pub async fn download_verified(
    client: &RequestClient,
    events: &EventBus,
    url: &str,
    dest: &Path,
    expected: Option<&Checksum>,
    expected_size: u64,
    notify: Option<ResponseNotifyOptions>,
) -> Result<(), RequestError> {
    // Callers fanning out over thousands of files pre-create the tree so this
    // stat avoids a blocking-pool round trip per file
    if let Some(parent) = dest.parent()
        && !parent.is_dir()
    {
        polyio::create_dir_all(parent).await?;
    }

    // Needs a fixed id up front or each retry opens a new UI entry
    let notify = notify.map(ResponseNotifyOptions::pinned);

    let mut attempt = 1;
    loop {
        match download_attempt(
            client,
            events,
            url,
            dest,
            expected,
            expected_size,
            notify.as_ref(),
            attempt < MAX_DOWNLOAD_ATTEMPTS,
        )
        .await
        {
            Ok(()) => break,
            Err(err) if attempt < MAX_DOWNLOAD_ATTEMPTS && worth_retrying(&err) => {
                tracing::warn!(
                    attempt,
                    %url,
                    "download failed, retrying: {err}"
                );
                tokio::time::sleep(retry_delay(attempt)).await;
                attempt += 1;
            }
            Err(err) => return Err(err),
        }
    }

    Ok(())
}

/// The progress child is shared across attempts and reports absolute byte
/// counts so a restarted attempt rewinds its bar rather than double-counting
#[allow(clippy::too_many_arguments)]
async fn download_attempt(
    client: &RequestClient,
    events: &EventBus,
    url: &str,
    dest: &Path,
    expected: Option<&Checksum>,
    expected_size: u64,
    notify: Option<&ResponseNotifyOptions>,
    allow_ranged: bool,
) -> Result<(), RequestError> {
    let child = notify.and_then(ResponseNotifyOptions::child);
    if let Some(child) = child {
        child.set_phase(TaskPhase::Downloading);
    }

    let started = Instant::now();
    let response = client.send(file_request(url, None)?).await?;
    let version = response.version();
    let remote = response.remote_addr();

    // Without this an error body is written to disk under the requested file's name
    let status = response.status();
    if !status.is_success() {
        let bytes = response.bytes().await?;
        return Err(RequestError::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
            snippet: crate::error::body_snippet(&bytes),
        });
    }

    if allow_ranged
        && let Some(expected) = expected
        && let Some(length) = response.content_length()
        && length >= RESUMABLE_MIN_SIZE
        && (expected_size == 0 || expected_size == length)
        && response
            .headers()
            .get(ACCEPT_RANGES)
            .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"bytes"))
    {
        return download_ranged(
            client, events, url, dest, expected, length, response, notify,
        )
        .await;
    }

    // Prefer the manifest size Content-Length is absent for compressed or chunked bodies
    let declared = if expected_size > 0 {
        Some(expected_size)
    } else {
        response.content_length()
    };
    let total = declared.unwrap_or(0).max(1);

    let options = ResponseOptions {
        notify: notify.cloned(),
    };
    let stream = response.stream(options, events).await?;

    let tmp = polyio::temp_sibling(dest);
    let mut hasher = expected.map(|expected| ChecksumStream::new(expected.algorithm));
    let mut received = 0u64;
    let deadline = started + SINGLE_STREAM_TIMEOUT;
    {
        let stream = futures_lite::StreamExt::map(stream, |item| {
            if let Ok(chunk) = &item {
                if Instant::now() >= deadline {
                    return Err(RequestError::IncompleteBody {
                        source_desc: format!("{url} (timed out)"),
                        expected: declared.unwrap_or(0),
                        actual: received,
                    });
                }
                received += chunk.len() as u64;
                if let Some(hasher) = hasher.as_mut() {
                    hasher.update(chunk);
                }
            }
            item
        });
        let stream = std::pin::pin!(stream);
        // Writes to a scratch sibling and renames so `dest` is never truncated
        let written = polyio::write_stream(&tmp, stream, Some(total)).await;
        let secs = started.elapsed().as_secs_f64();
        tracing::debug!(
            bytes = received,
            secs,
            kib_per_sec = (received as f64 / 1024.0 / secs.max(0.001)) as u64,
            ?version,
            ?remote,
            ok = written.is_ok(),
            "download body ended"
        );
        written?;
    }

    if let (Some(expected), Some(hasher)) = (expected, hasher) {
        if let Some(child) = child {
            child.set_phase(TaskPhase::Verifying);
        }

        let actual = hasher.finish();
        if !expected.matches(&actual) {
            let _ = polyio::remove_file(&tmp).await;
            return Err(RequestError::HashMismatch {
                source_desc: dest.display().to_string(),
                expected: format!("{} {}", expected.algorithm.name(), expected.hex),
                actual,
            });
        }
    } else if let Some(declared) = declared
        && received != declared
    {
        // With no hash the byte count is the only evidence the transfer finished
        let _ = polyio::remove_file(&tmp).await;
        return Err(RequestError::IncompleteBody {
            source_desc: dest.display().to_string(),
            expected: declared,
            actual: received,
        });
    }

    if let Err(err) = polyio::rename(&tmp, dest).await {
        let _ = polyio::remove_file(&tmp).await;
        return Err(err.into());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_ranged(
    client: &RequestClient,
    events: &EventBus,
    url: &str,
    dest: &Path,
    expected: &Checksum,
    length: u64,
    probe: reqwest::Response,
    notify: Option<&ResponseNotifyOptions>,
) -> Result<(), RequestError> {
    let started = Instant::now();
    let tmp = polyio::temp_sibling(dest);
    let mut scratch = Scratch {
        path: Some(&tmp),
        dropped: Arc::default(),
    };
    let parts = if length >= RANGED_MIN_SIZE {
        RANGED_PARTS
    } else {
        1
    };
    let mut probe = Some(probe);
    let received = AtomicU64::new(0);

    let result = async {
        let dropped = scratch.dropped.clone();
        let path = tmp.clone();
        tokio::task::spawn_blocking(move || create_scratch(&path, length, &dropped))
            .await
            .map_err(std::io::Error::other)
            .flatten()
            .map_err(polyio::IOError::from)?;

        let part = length.div_ceil(parts);
        let parts = (0..parts)
            .map(|i| (i * part, ((i + 1) * part).min(length)))
            .filter(|(start, end)| start < end)
            .map(|(start, end)| {
                let first = if start == 0 { probe.take() } else { None };
                download_part(
                    client, events, url, &tmp, start, end, length, &received, first, notify,
                )
            });
        futures_util::future::try_join_all(parts).await?;

        if let Some(child) = notify.and_then(ResponseNotifyOptions::child) {
            child.set_phase(TaskPhase::Verifying);
        }
        let actual = checksum_file(&tmp, expected).await?;
        if !expected.matches(&actual) {
            return Err(RequestError::HashMismatch {
                source_desc: dest.display().to_string(),
                expected: format!("{} {}", expected.algorithm.name(), expected.hex),
                actual,
            });
        }

        polyio::rename(&tmp, dest).await?;
        scratch.path = None;
        Ok(())
    }
    .await;

    let bytes = received.load(Ordering::Relaxed);
    let secs = started.elapsed().as_secs_f64();
    tracing::debug!(
        bytes,
        secs,
        kib_per_sec = (bytes as f64 / 1024.0 / secs.max(0.001)) as u64,
        parts,
        ok = result.is_ok(),
        "ranged download ended"
    );

    result
}

struct Scratch<'a> {
    path: Option<&'a Path>,
    dropped: Arc<AtomicBool>,
}

fn create_scratch(path: &Path, length: u64, dropped: &AtomicBool) -> std::io::Result<()> {
    let created = std::fs::File::create(path).and_then(|file| file.set_len(length));
    if dropped.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(path);
    }
    created
}

impl Drop for Scratch<'_> {
    fn drop(&mut self) {
        if let Some(path) = self.path {
            self.dropped.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn serves_range(response: &reqwest::Response, start: u64, length: u64) -> bool {
    response.status() == StatusCode::PARTIAL_CONTENT
        && response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes "))
            .and_then(|value| {
                let (range, total) = value.split_once('/')?;
                let (first, _) = range.split_once('-')?;
                Some((first.parse::<u64>().ok()?, total.parse::<u64>().ok()?))
            })
            .is_some_and(|(first, total)| first == start && total == length)
}

#[allow(clippy::too_many_arguments)]
async fn download_part(
    client: &RequestClient,
    events: &EventBus,
    url: &str,
    tmp: &Path,
    start: u64,
    end: u64,
    length: u64,
    received: &AtomicU64,
    first: Option<reqwest::Response>,
    notify: Option<&ResponseNotifyOptions>,
) -> Result<(), RequestError> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(tmp)
        .await
        .map_err(polyio::IOError::from)?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(polyio::IOError::from)?;
    let mut writer = tokio::io::BufWriter::with_capacity(256 * 1024, file);

    let wanted = end - start;
    let mut got = 0u64;
    let mut first = first;
    let mut reconnects = 0u32;
    let mut last_report = Instant::now();
    while got < wanted {
        let response = match first.take() {
            Some(response) => response,
            None => {
                let response = client
                    .send(file_request(url, Some((start + got, end)))?)
                    .await?;
                if !serves_range(&response, start + got, length) {
                    return Err(RequestError::IncompleteBody {
                        source_desc: format!(
                            "{url} (range {}-{end} got {})",
                            start + got,
                            response.status()
                        ),
                        expected: wanted,
                        actual: got,
                    });
                }
                response
            }
        };

        let mut stream = response.bytes_stream();
        let mut window_start = Instant::now();
        let mut window_bytes = 0u64;
        let mut slow = false;
        while got < wanted {
            let window_end = window_start + SLOW_WINDOW;
            let next =
                tokio::time::timeout_at(tokio::time::Instant::from_std(window_end), stream.next())
                    .await;
            match next {
                Ok(None) => break,
                Ok(Some(chunk)) => {
                    let chunk = chunk?;
                    let take = chunk.len().min((wanted - got) as usize);
                    writer
                        .write_all(&chunk[..take])
                        .await
                        .map_err(polyio::IOError::from)?;
                    got += take as u64;
                    window_bytes += take as u64;
                    let total = received.fetch_add(take as u64, Ordering::Relaxed) + take as u64;
                    if let Some(notify) = notify
                        && last_report.elapsed() >= RANGED_PROGRESS_INTERVAL
                    {
                        last_report = Instant::now();
                        notify.report(events, total, length);
                    }
                }
                Err(_window_elapsed) => {}
            }
            if Instant::now() >= window_end {
                let floor = SLOW_FLOOR_BYTES_PER_SEC * SLOW_WINDOW.as_millis() as u64 / 1000;
                if window_bytes < floor && reconnects < MAX_SLOW_RECONNECTS {
                    slow = true;
                    break;
                }
                window_start = Instant::now();
                window_bytes = 0;
            }
        }

        if got >= wanted {
            break;
        }
        if !slow {
            return Err(RequestError::IncompleteBody {
                source_desc: format!("{url} (range {start}-{end})"),
                expected: wanted,
                actual: got,
            });
        }
        reconnects += 1;
        tracing::debug!(
            %url,
            start,
            end,
            got,
            reconnects,
            "connection under the speed floor, resuming on a new one"
        );
    }
    writer.flush().await.map_err(polyio::IOError::from)?;

    if let Some(notify) = notify {
        notify.report(events, received.load(Ordering::Relaxed), length);
    }
    Ok(())
}

async fn checksum_file(path: &Path, expected: &Checksum) -> Result<String, RequestError> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(polyio::IOError::from)?;
    let mut hasher = ChecksumStream::new(expected.algorithm);
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buf).await.map_err(polyio::IOError::from)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finish())
}

#[tracing::instrument(level = "debug", skip(client, events, progress), fields(%url))]
pub async fn fetch_verified(
    client: &RequestClient,
    events: &EventBus,
    url: &str,
    expected_sha1: &str,
    progress: Option<GroupedProgressChild>,
) -> Result<Vec<u8>, RequestError> {
    let request = reqwest::Request::new(Method::GET, url.parse()?);
    let response = client.send(request).await?;

    let options = ResponseOptions {
        notify: progress.clone().map(ResponseNotifyOptions::grouped),
    };
    let stream = response.stream(options, events).await?;

    let mut stream = std::pin::pin!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
    }

    if let Some(child) = &progress {
        child.set_phase(TaskPhase::Verifying);
    }

    let actual = polyio::sha1_bytes(&bytes);
    if normalize_hash(&actual) != normalize_hash(expected_sha1) {
        return Err(RequestError::HashMismatch {
            source_desc: url.to_string(),
            expected: expected_sha1.to_string(),
            actual,
        });
    }

    if let Some(child) = progress {
        child.finish();
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use oneclient_events::{
        Event, GroupedProgressEvent, GroupedProgressSession, ProgressEvent, TaskCategory,
    };
    use tokio::io::{AsyncBufReadExt, BufReader};

    use super::*;

    async fn serve(
        body: Arc<Vec<u8>>,
        ranged_hits: Arc<AtomicUsize>,
        stall_probe: bool,
        pause: Option<Duration>,
    ) -> String {
        serve_labelled(
            body,
            ranged_hits,
            Arc::default(),
            stall_probe,
            pause,
            |a, b, length| format!("bytes {a}-{b}/{length}"),
        )
        .await
    }

    async fn serve_labelled(
        body: Arc<Vec<u8>>,
        ranged_hits: Arc<AtomicUsize>,
        probes: Arc<AtomicUsize>,
        stall_probe: bool,
        pause: Option<Duration>,
        label: fn(usize, usize, usize) -> String,
    ) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let body = body.clone();
                let ranged_hits = ranged_hits.clone();
                let probes = probes.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(socket);
                    let mut range = None;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.unwrap() == 0 {
                            return;
                        }
                        let line = line.trim_end();
                        if line.is_empty() {
                            break;
                        }
                        if let Some(spec) = line.to_ascii_lowercase().strip_prefix("range: bytes=")
                        {
                            let (a, b) = spec.split_once('-').unwrap();
                            range =
                                Some((a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()));
                        }
                    }
                    let (status, slice, content_range) = match range {
                        Some((a, b)) => {
                            ranged_hits.fetch_add(1, Ordering::Relaxed);
                            (
                                "206 Partial Content",
                                &body[a..=b],
                                format!("Content-Range: {}\r\n", label(a, b, body.len())),
                            )
                        }
                        None => {
                            probes.fetch_add(1, Ordering::Relaxed);
                            ("200 OK", &body[..], String::new())
                        }
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{content_range}Accept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        slice.len()
                    );
                    let mut socket = reader.into_inner();
                    let _ = socket.write_all(head.as_bytes()).await;
                    if stall_probe && range.is_none() {
                        let _ = socket.write_all(&slice[..1000]).await;
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                        return;
                    }
                    if let Some(pause) = pause {
                        let _ = socket.write_all(&slice[..1000]).await;
                        tokio::time::sleep(pause).await;
                        let _ = socket.write_all(&slice[1000..]).await;
                        return;
                    }
                    let _ = socket.write_all(slice).await;
                });
            }
        });
        format!("http://{addr}/big.jar")
    }

    #[tokio::test]
    async fn a_download_leaves_finishing_its_progress_to_the_caller() {
        let body = Arc::new(b"small file".to_vec());
        let url = serve(body.clone(), Arc::new(AtomicUsize::new(0)), false, None).await;
        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let session = GroupedProgressSession::start(&events, "test");
        let child = session.child("small.jar", 1, TaskCategory::Packages);
        let dest = std::env::temp_dir().join(format!("oneclient-owned-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let result = download_verified(
            &client,
            &events,
            &url,
            &dest,
            Some(&expected),
            0,
            Some(ResponseNotifyOptions::grouped(child.clone())),
        )
        .await;
        let _ = tokio::fs::remove_file(&dest).await;
        result.unwrap();

        while let Ok(event) = rx.try_recv() {
            assert!(
                !matches!(
                    event,
                    Event::Progress(ProgressEvent::Grouped(
                        GroupedProgressEvent::FinishChild { .. }
                    ))
                ),
                "the caller still holds the child"
            );
        }
    }

    #[tokio::test]
    async fn large_files_download_in_ranges_and_verify() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RANGED_MIN_SIZE as usize + 12345)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let ranged_hits = Arc::new(AtomicUsize::new(0));
        let url = serve(body.clone(), ranged_hits.clone(), false, None).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-ranged-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        download_verified(&client, &events, &url, &dest, Some(&expected), 0, None)
            .await
            .unwrap();

        let written = tokio::fs::read(&dest).await.unwrap();
        let _ = tokio::fs::remove_file(&dest).await;
        assert!(written == *body, "reassembled file differs from the source");
        assert_eq!(
            ranged_hits.load(Ordering::Relaxed),
            RANGED_PARTS as usize - 1
        );
    }

    #[tokio::test]
    async fn ranged_progress_counts_every_part() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RANGED_MIN_SIZE as usize + 12345)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let ranged_hits = Arc::new(AtomicUsize::new(0));
        let pause = RANGED_PROGRESS_INTERVAL + Duration::from_millis(50);
        let url = serve(body.clone(), ranged_hits.clone(), false, Some(pause)).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest = std::env::temp_dir().join(format!(
            "oneclient-ranged-progress-{}.jar",
            std::process::id()
        ));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let result = download_verified(
            &client,
            &events,
            &url,
            &dest,
            Some(&expected),
            0,
            Some(ResponseNotifyOptions::standalone("big.jar").pinned()),
        )
        .await;
        let _ = tokio::fs::remove_file(&dest).await;
        result.unwrap();

        let mut reported = Vec::new();
        while let Ok(event) = rx.try_recv() {
            if let Event::Progress(ProgressEvent::Update { current, total, .. }) = event {
                assert_eq!(total, body.len() as u64);
                reported.push(current);
            }
        }
        assert!(ranged_hits.load(Ordering::Relaxed) >= RANGED_PARTS as usize - 1);
        assert!(
            reported
                .iter()
                .any(|&current| current > 0 && current < body.len() as u64),
            "no intermediate progress in {reported:?}"
        );
        assert_eq!(reported.last(), Some(&(body.len() as u64)));
    }

    #[tokio::test]
    async fn a_trickling_connection_is_resumed_on_a_new_one() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RESUMABLE_MIN_SIZE as usize * 2)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let ranged_hits = Arc::new(AtomicUsize::new(0));
        let url = serve(body.clone(), ranged_hits.clone(), true, None).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-trickle-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let started = Instant::now();
        let download = download_verified(&client, &events, &url, &dest, Some(&expected), 0, None);
        let finished = tokio::time::timeout(Duration::from_secs(10), download).await;

        let written = tokio::fs::read(&dest).await;
        let _ = tokio::fs::remove_file(&dest).await;
        finished
            .expect("stalled connection was never replaced")
            .unwrap();
        assert!(
            written.unwrap() == *body,
            "resumed file differs from the source"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
        assert_eq!(ranged_hits.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_link_slow_on_every_connection_stops_reconnecting() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RESUMABLE_MIN_SIZE as usize * 2)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let ranged_hits = Arc::new(AtomicUsize::new(0));
        let url = serve(
            body.clone(),
            ranged_hits.clone(),
            false,
            Some(SLOW_WINDOW * 2 + Duration::from_millis(100)),
        )
        .await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-slow-link-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let download = download_verified(&client, &events, &url, &dest, Some(&expected), 0, None);
        let finished = tokio::time::timeout(Duration::from_secs(10), download).await;

        let written = tokio::fs::read(&dest).await;
        let _ = tokio::fs::remove_file(&dest).await;
        finished.expect("kept reconnecting").unwrap();
        assert!(written.unwrap() == *body);
        assert_eq!(ranged_hits.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn a_ranged_body_that_fails_verification_is_not_published() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RESUMABLE_MIN_SIZE as usize * 2)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let url = serve(body, Arc::new(AtomicUsize::new(0)), false, None).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-corrupt-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(b"another file"));

        let err = download_verified(&client, &events, &url, &dest, Some(&expected), 0, None)
            .await
            .unwrap_err();

        assert!(matches!(err, RequestError::HashMismatch { .. }), "{err}");
        assert!(
            !dest.exists(),
            "a file that failed verification was published"
        );
    }

    #[tokio::test]
    async fn a_cancelled_ranged_download_leaves_no_scratch_file() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RESUMABLE_MIN_SIZE as usize * 2)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let url = serve(body.clone(), Arc::new(AtomicUsize::new(0)), true, None).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dir = std::env::temp_dir().join(format!("oneclient-cancel-{}", std::process::id()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let dest = dir.join("big.jar");
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let download = download_verified(&client, &events, &url, &dest, Some(&expected), 0, None);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), download)
                .await
                .is_err()
        );

        let mut entries = tokio::fs::read_dir(&dir).await.unwrap();
        let left = entries
            .next_entry()
            .await
            .unwrap()
            .map(|entry| entry.path());
        let _ = tokio::fs::remove_dir_all(&dir).await;
        assert_eq!(left, None);
    }

    #[test]
    fn a_download_dropped_while_its_scratch_is_created_leaves_no_file() {
        let dir = std::env::temp_dir().join(format!("oneclient-scratch-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tmp = dir.join("big.jar.tmp");
        let scratch = Scratch {
            path: Some(&tmp),
            dropped: Arc::default(),
        };
        let dropped = scratch.dropped.clone();

        drop(scratch);
        let created = create_scratch(&tmp, 1024, &dropped);

        let left = tmp.exists();
        let _ = std::fs::remove_dir_all(&dir);
        created.unwrap();
        assert!(!left, "the scratch file outlived its download");
    }

    #[tokio::test]
    async fn a_single_stream_cannot_outlast_the_overall_limit() {
        let body = Arc::new(vec![7u8; 4000]);
        let url = serve(
            body.clone(),
            Arc::default(),
            false,
            Some(SINGLE_STREAM_TIMEOUT + Duration::from_millis(300)),
        )
        .await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-single-limit-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let result = download_attempt(
            &client,
            &events,
            &url,
            &dest,
            Some(&expected),
            0,
            None,
            true,
        )
        .await;

        let written = dest.exists();
        let _ = tokio::fs::remove_file(&dest).await;
        assert!(
            matches!(result, Err(RequestError::IncompleteBody { .. })),
            "{result:?}"
        );
        assert!(!written);
    }

    async fn serve_ignoring_ranges(body: Arc<Vec<u8>>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let body = body.clone();
                tokio::spawn(async move {
                    let mut reader = BufReader::new(socket);
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        if line.trim_end().is_empty() {
                            break;
                        }
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let mut socket = reader.into_inner();
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });
        format!("http://{addr}/big.jar")
    }

    #[tokio::test]
    async fn a_server_that_mishandles_ranges_still_gets_the_file_in_one_piece() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RANGED_MIN_SIZE as usize + 12345)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let url = serve_ignoring_ranges(body.clone()).await;

        let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let events = EventBus::new(tx);
        let dest =
            std::env::temp_dir().join(format!("oneclient-no-ranges-{}.jar", std::process::id()));
        let expected = Checksum::sha1(polyio::sha1_bytes(&body));

        let result =
            download_verified(&client, &events, &url, &dest, Some(&expected), 0, None).await;

        let written = tokio::fs::read(&dest).await;
        let _ = tokio::fs::remove_file(&dest).await;
        result.unwrap();
        assert!(written.unwrap() == *body);
    }

    #[tokio::test]
    async fn a_mislabelled_range_is_not_trusted() {
        let body: Arc<Vec<u8>> = Arc::new(
            (0..RANGED_MIN_SIZE as usize + 12345)
                .map(|i| (i % 251) as u8)
                .collect(),
        );
        let labels: [(&str, fn(usize, usize, usize) -> String); 2] = [
            ("start", |a, b, length| {
                format!("bytes {}-{b}/{length}", a + 1)
            }),
            ("total", |a, b, length| {
                format!("bytes {a}-{b}/{}", length + 1)
            }),
        ];
        for (name, label) in labels {
            let probes = Arc::new(AtomicUsize::new(0));
            let url = serve_labelled(
                body.clone(),
                Arc::default(),
                probes.clone(),
                false,
                None,
                label,
            )
            .await;

            let client = RequestClient::new(crate::config::NetConfig::default()).unwrap();
            let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
            let events = EventBus::new(tx);
            let dest = std::env::temp_dir().join(format!(
                "oneclient-mislabelled-{name}-{}.jar",
                std::process::id()
            ));
            let expected = Checksum::sha1(polyio::sha1_bytes(&body));

            let result =
                download_verified(&client, &events, &url, &dest, Some(&expected), 0, None).await;

            let written = tokio::fs::read(&dest).await;
            let _ = tokio::fs::remove_file(&dest).await;
            result.unwrap();
            assert!(written.unwrap() == *body, "{name}");
            assert_eq!(
                probes.load(Ordering::Relaxed),
                MAX_DOWNLOAD_ATTEMPTS as usize,
                "a part under a wrong {name} was used"
            );
        }
    }
}
