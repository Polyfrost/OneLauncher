use console::{Style, measure_text_width, pad_str};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reply {
    Out(String),
    Err(String),
    Page(String),
    Exit(i32),
    Released,
}

#[derive(Debug, Clone)]
pub struct Output(UnboundedSender<Reply>);

impl Output {
    #[must_use]
    pub fn channel() -> (Self, UnboundedReceiver<Reply>) {
        let (tx, rx) = unbounded_channel();
        (Self(tx), rx)
    }

    pub fn line(&self, text: impl Into<String>) {
        let _ = self.0.send(Reply::Out(text.into()));
    }

    pub fn error(&self, text: impl Into<String>) {
        let _ = self.0.send(Reply::Err(text.into()));
    }

    pub fn page(&self, text: impl Into<String>) {
        let _ = self.0.send(Reply::Page(text.into()));
    }

    pub fn exit(&self, code: i32) {
        let _ = self.0.send(Reply::Exit(code));
    }

    pub fn fail(&self, text: impl Into<String>) {
        self.error(text);
        self.exit(1);
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

fn paint(style: Style, text: impl std::fmt::Display) -> String {
    style.force_styling(true).apply_to(text).to_string()
}

pub(crate) fn bold(text: impl std::fmt::Display) -> String {
    paint(Style::new().bold(), text)
}

pub(crate) fn dim(text: impl std::fmt::Display) -> String {
    paint(Style::new().dim(), text)
}

pub(crate) fn accent(text: impl std::fmt::Display) -> String {
    paint(Style::new().cyan(), text)
}

pub(crate) fn good(text: impl std::fmt::Display) -> String {
    paint(Style::new().green(), text)
}

pub(crate) fn warn(text: impl std::fmt::Display) -> String {
    paint(Style::new().yellow(), text)
}

pub(crate) fn bad(text: impl std::fmt::Display) -> String {
    paint(Style::new().red(), text)
}

pub(crate) fn heading(text: impl std::fmt::Display) -> String {
    paint(Style::new().bold().cyan(), text)
}

pub(crate) fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(measure_text_width(cell));
        }
    }

    let render = |cells: &mut dyn Iterator<Item = String>| {
        cells
            .zip(&widths)
            .map(|(cell, width)| {
                pad_str(&cell, *width, console::Alignment::Left, None).into_owned()
            })
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };

    let mut lines = vec![render(&mut headers.iter().map(bold))];
    for row in rows {
        lines.push(render(&mut row.iter().cloned()));
    }
    lines.join("\n")
}

pub(crate) fn when(time: chrono::DateTime<chrono::Utc>) -> String {
    time.with_timezone(&chrono::Local)
        .format("%Y-%m-%d %H:%M")
        .to_string()
}

pub(crate) fn count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

pub(crate) fn bytes(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.1} KiB", n as f64 / 1024.0),
        1_048_576..1_073_741_824 => format!("{:.1} MiB", n as f64 / 1_048_576.0),
        _ => format!("{:.1} GiB", n as f64 / 1_073_741_824.0),
    }
}

pub(crate) fn duration(d: std::time::Duration) -> String {
    let minutes = d.as_secs() / 60;
    match minutes {
        _ if d.is_zero() => "none".to_string(),
        0 => "<1m".to_string(),
        1..60 => format!("{minutes}m"),
        _ => format!("{}h {}m", minutes / 60, minutes % 60),
    }
}
