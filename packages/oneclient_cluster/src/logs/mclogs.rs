use std::path::Path;

use serde::Deserialize;

use crate::error::ClusterResult;
use oneclient_net::RequestError;

use super::manage::{ensure_allowed, read_file_string};
use super::{LogTrim, LogsError, MclogsUploadResponse};

const MCLOGS_URL: &str = "https://api.mclo.gs/1/log";

const MAX_LINES: usize = 25_000;
const MAX_BYTES: usize = 10 * 1024 * 1024;
const HEAD_LINES: usize = 10_000;
const MARKER_RESERVE: usize = 128;

#[derive(Deserialize)]
struct MclogsResponse {
    success: bool,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    raw: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[tracing::instrument]
pub async fn log_exceeds_upload_limit(path: &Path) -> ClusterResult<bool> {
    let path = ensure_allowed(path)?;
    let content = read_file_string(&path).await?;
    Ok(exceeds_limits(
        &super::censor(&content).lines().collect::<Vec<_>>(),
    ))
}

#[tracing::instrument(skip(net))]
pub async fn upload_log_at(
    net: &oneclient_net::RequestClient,
    path: &Path,
    trim: LogTrim,
) -> ClusterResult<MclogsUploadResponse> {
    let path = ensure_allowed(path)?;
    let content = super::censor(&read_file_string(&path).await?).into_owned();

    let trimmed = trim_log(&content, trim);
    let truncated = trimmed.is_some();
    let content = trimmed.unwrap_or(content);

    let response = net
        .http()
        .post(MCLOGS_URL)
        .form(&[("content", content.as_str())])
        .send()
        .await
        .map_err(RequestError::ReqwestError)?;

    let status = response.status();
    let bytes = response.bytes().await.map_err(RequestError::ReqwestError)?;

    if !status.is_success() {
        let reason = serde_json::from_slice::<MclogsResponse>(&bytes)
            .ok()
            .and_then(|r| r.error)
            .unwrap_or_else(|| status.to_string());
        tracing::warn!(
            %status,
            body = %String::from_utf8_lossy(&bytes[..bytes.len().min(512)]),
            "mclogs upload rejected"
        );
        return Err(LogsError::Upload(reason).into());
    }

    let parsed: MclogsResponse = serde_json::from_slice(&bytes)?;

    if !parsed.success {
        let reason = parsed.error.unwrap_or_else(|| "unknown error".into());
        tracing::warn!(reason = %reason, "mclogs upload failed");
        return Err(LogsError::Upload(reason).into());
    }

    tracing::info!(
        url = parsed.url.as_deref().unwrap_or(""),
        "uploaded log to mclo.gs"
    );

    Ok(MclogsUploadResponse {
        id: parsed.id.unwrap_or_default(),
        url: parsed.url.unwrap_or_default(),
        raw: parsed.raw.unwrap_or_default(),
        truncated,
    })
}

fn form_encoded_byte_len(byte: u8) -> usize {
    match byte {
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' | b' ' => 1,
        _ => 3,
    }
}

fn form_encoded_len(bytes: &[u8]) -> usize {
    bytes.iter().map(|&b| form_encoded_byte_len(b)).sum()
}

fn line_cost(line: &str) -> usize {
    form_encoded_len(line.as_bytes()) + form_encoded_byte_len(b'\n')
}

fn exceeds_limits(lines: &[&str]) -> bool {
    lines.len() > MAX_LINES || lines.iter().map(|l| line_cost(l)).sum::<usize>() > MAX_BYTES
}

fn fit_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    max_lines: usize,
    max_bytes: usize,
) -> (usize, usize) {
    let (mut count, mut bytes) = (0, 0);
    for line in lines.take(max_lines) {
        let cost = line_cost(line);
        if bytes + cost > max_bytes {
            break;
        }
        bytes += cost;
        count += 1;
    }
    (count, bytes)
}

fn trim_log(content: &str, trim: LogTrim) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    if !exceeds_limits(&lines) {
        return None;
    }

    if trim == LogTrim::KeepStart {
        let (head, _) = fit_lines(lines.iter().copied(), MAX_LINES, MAX_BYTES);
        return Some(if head == 0 {
            clip_start(lines[0], MAX_BYTES).to_string()
        } else {
            lines[..head].join("\n")
        });
    }

    let max_lines = MAX_LINES - 1;
    let max_bytes = MAX_BYTES - MARKER_RESERVE;
    let (head, mut used) = fit_lines(lines.iter().copied(), HEAD_LINES, max_bytes / 2);
    let clipped_head = (head == 0).then(|| clip_start(lines[0], max_bytes / 2));
    used += clipped_head.map_or(0, line_cost);

    let tail_start = head.max(1);
    let (tail, _) = fit_lines(
        lines[tail_start..].iter().rev().copied(),
        max_lines - tail_start,
        max_bytes - used,
    );
    let clipped_tail = (tail == 0).then(|| clip_end(lines[lines.len() - 1], max_bytes - used));

    let marker = format!("... {} lines omitted ...", lines.len() - head - tail);
    let mut kept = lines[..head].to_vec();
    kept.extend(clipped_head);
    kept.push(&marker);
    kept.extend(clipped_tail);
    kept.extend_from_slice(&lines[lines.len() - tail..]);
    Some(kept.join("\n"))
}

fn char_cost(c: char) -> usize {
    form_encoded_len(c.encode_utf8(&mut [0; 4]).as_bytes())
}

fn clip_start(line: &str, max_bytes: usize) -> &str {
    let mut used = 0;
    for (index, c) in line.char_indices() {
        used += char_cost(c);
        if used > max_bytes {
            return &line[..index];
        }
    }
    line
}

fn clip_end(line: &str, max_bytes: usize) -> &str {
    let mut used = 0;
    for (index, c) in line.char_indices().rev() {
        used += char_cost(c);
        if used > max_bytes {
            return &line[index + c.len_utf8()..];
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered_log(count: usize) -> String {
        (0..count)
            .map(|i| format!("line{i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn leaves_small_logs_alone() {
        let log = numbered_log(MAX_LINES);
        assert_eq!(trim_log(&log, LogTrim::KeepStartAndEnd), None);
        assert_eq!(trim_log(&log, LogTrim::KeepStart), None);
    }

    #[test]
    fn drops_the_middle_of_long_logs() {
        let trimmed = trim_log(&numbered_log(30_000), LogTrim::KeepStartAndEnd).unwrap();
        let lines: Vec<&str> = trimmed.lines().collect();

        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines[0], "line0");
        assert_eq!(lines[HEAD_LINES - 1], "line9999");
        assert_eq!(lines[HEAD_LINES], "... 5001 lines omitted ...");
        assert_eq!(lines[HEAD_LINES + 1], "line15001");
        assert_eq!(lines[MAX_LINES - 1], "line29999");
    }

    #[test]
    fn keep_start_drops_the_end_of_long_logs() {
        let trimmed = trim_log(&numbered_log(30_000), LogTrim::KeepStart).unwrap();
        let lines: Vec<&str> = trimmed.lines().collect();

        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines[0], "line0");
        assert_eq!(lines[MAX_LINES - 1], "line24999");
    }

    #[test]
    fn both_modes_respect_the_size_limit() {
        let log = (0..20)
            .map(|i| format!("{i}a{}", "a".repeat(1024 * 1024)))
            .collect::<Vec<_>>()
            .join("\n");

        let both = trim_log(&log, LogTrim::KeepStartAndEnd).unwrap();
        assert!(form_encoded_len(both.as_bytes()) <= MAX_BYTES);
        assert!(both.starts_with("0a"));
        assert!(both.lines().last().unwrap().starts_with("19a"));

        let start = trim_log(&log, LogTrim::KeepStart).unwrap();
        assert!(form_encoded_len(start.as_bytes()) <= MAX_BYTES);
        assert!(start.starts_with("0a"));
    }

    #[test]
    fn clips_a_single_line_that_is_too_long() {
        let log = "é".repeat(2 * 1024 * 1024);

        let both = trim_log(&log, LogTrim::KeepStartAndEnd).unwrap();
        let lines: Vec<&str> = both.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with('é'));
        assert_eq!(lines[1], "... 1 lines omitted ...");
        assert!(lines[2].ends_with('é'));
        assert!(form_encoded_len(both.as_bytes()) <= MAX_BYTES);

        let start = trim_log(&log, LogTrim::KeepStart).unwrap();
        assert!(start.starts_with('é'));
        assert!(form_encoded_len(start.as_bytes()) <= MAX_BYTES);
    }

    #[test]
    fn keeps_part_of_a_last_line_that_is_too_long() {
        let log = format!("first\n{}", "z".repeat(MAX_BYTES + 1));

        let both = trim_log(&log, LogTrim::KeepStartAndEnd).unwrap();
        let lines: Vec<&str> = both.lines().collect();
        assert_eq!(lines[0], "first");
        assert_eq!(lines[1], "... 1 lines omitted ...");
        assert!(lines[2].starts_with('z'));
        assert!(form_encoded_len(both.as_bytes()) <= MAX_BYTES);
    }
}
