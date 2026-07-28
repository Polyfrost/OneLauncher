use super::LogLevel;

pub fn parse_level(line: &str) -> Option<LogLevel> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'['
            && let Some(rel_end) = line[i + 1..].find(']')
        {
            let seg = &line[i + 1..i + 1 + rel_end];
            if let Some(level) = level_from_segment(seg) {
                return Some(level);
            }
            i = i + 1 + rel_end + 1;
            continue;
        }
        i += 1;
    }
    None
}

fn level_from_segment(seg: &str) -> Option<LogLevel> {
    let token = seg.rsplit('/').next().unwrap_or(seg).trim();
    LogLevel::from_token(token)
}
