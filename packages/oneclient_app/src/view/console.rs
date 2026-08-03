//! The live launcher log console, in a window of its own.
//!
//! Freya gives every window its own component tree, so this one shares nothing
//! with the main window — no radio station, no query storage. All it needs is
//! the process-wide channel in [`oneclient_core::logger::console`], and that
//! channel only carries anything while this window is up.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use freya::prelude::*;
use freya::winit::window::WindowId;
use oneclient_core::logger;
use oneclient_core::logger::console::{self, ConsoleLine};
use tokio::sync::broadcast::error::RecvError;
use tracing::Level;

use crate::components::{Button, Icon, IconType, LogViewer, Segment, SegmentedControl, TextInput};
use crate::theme::colors;

const WINDOW_TITLE: &str = "OneClient — Log Console";

/// How many lines the console keeps.
///
/// Old lines are dropped from the front rather than kept forever: this window
/// can be left open for an entire session, and a launcher that leaks memory
/// while you watch it for leaks is not much use.
const MAX_LINES: usize = 5_000;

/// Where the console window is in its lifecycle.
///
/// `Opening` is not set by the task that asks for the window but cleared by the
/// window itself, so a task cancelled mid-launch — the settings page being
/// navigated away from, say — cannot strand the state.
enum ConsoleWindow {
    Closed,
    Opening,
    Open(WindowId),
}

static CONSOLE_WINDOW: Mutex<ConsoleWindow> = Mutex::new(ConsoleWindow::Closed);

/// Opens the log console, or focuses it if it is already up.
pub fn open_log_console() {
    {
        let mut state = CONSOLE_WINDOW.lock().expect("console window state");
        match *state {
            ConsoleWindow::Open(id) => {
                Platform::get().focus_window(Some(id));
                return;
            }
            ConsoleWindow::Opening => return,
            ConsoleWindow::Closed => *state = ConsoleWindow::Opening,
        }
    }

    // Taken here, in the caller's scope: the future below runs detached, and
    // `Platform::get()` needs a component scope to resolve against.
    let platform = Platform::get();
    spawn(async move {
        platform.launch_window(window_config()).await;
    });
}

fn window_config() -> WindowConfig {
    WindowConfig::new_app(LogConsoleApp)
        .with_title(WINDOW_TITLE)
        .with_app_id(crate::constants::WINDOW_APP_ID)
        .with_icon(LaunchConfig::window_icon(include_bytes!(
            "../../icons/128x128.png"
        )))
        .with_size(1000., 640.)
        .with_min_size(520., 320.)
        .with_background(colors::page())
        // Native decorations, unlike the main window's custom titlebar. This is
        // a developer tool, and a second bespoke titlebar is not worth the code.
        .with_decorations(true)
        .with_window_handle(|window| {
            *CONSOLE_WINDOW.lock().expect("console window state") =
                ConsoleWindow::Open(window.id());
        })
        .with_on_close(|_, _| {
            *CONSOLE_WINDOW.lock().expect("console window state") = ConsoleWindow::Closed;
            CloseDecision::Close
        })
}

struct LogConsoleApp;

impl App for LogConsoleApp {
    fn render(&self) -> impl IntoElement {
        LogConsole
    }
}

/// The lowest severity the console shows.
///
/// A floor rather than an exact match, unlike the cluster logs page: when you
/// are watching a stream go past, "warnings and worse" is the question you
/// actually have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Severity {
    All,
    Debug,
    Info,
    Warn,
    Error,
}

impl Severity {
    fn floor(self) -> Option<Level> {
        match self {
            Self::All => None,
            Self::Debug => Some(Level::DEBUG),
            Self::Info => Some(Level::INFO),
            Self::Warn => Some(Level::WARN),
            Self::Error => Some(Level::ERROR),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warn => "Warnings",
            Self::Error => "Errors",
        }
    }
}

#[derive(PartialEq)]
struct LogConsole;

impl Component for LogConsole {
    fn render(&self) -> impl IntoElement {
        let mut entries = use_state(VecDeque::<ConsoleLine>::new);
        let severity = use_state(|| Severity::All);
        let search = use_state(String::new);
        let directives = use_state(logger::active_directives);
        let directives_error = use_state(|| None::<String>);

        use_hook(move || {
            spawn(async move {
                // The subscription lives in this task and nowhere else, so when
                // the window closes and its tree is dropped the receiver goes
                // with it and capture switches itself off.
                let mut subscription = console::subscribe();
                tracing::info!("live log console attached");

                loop {
                    match subscription.recv().await {
                        Ok(line) => push(&mut entries, line),
                        Err(RecvError::Lagged(skipped)) => push(
                            &mut entries,
                            ConsoleLine {
                                level: Level::WARN,
                                text: format!(
                                    "[console] dropped {skipped} lines, the console fell behind"
                                )
                                .into(),
                            },
                        ),
                        Err(RecvError::Closed) => break,
                    }
                }
            })
        });

        let floor = severity.read().floor();
        let query = search.read().to_lowercase();
        let lines: Arc<Vec<Arc<str>>> = Arc::new(
            entries
                .read()
                .iter()
                .filter(|line| floor.is_none_or(|floor| line.level <= floor))
                .filter(|line| query.is_empty() || line.text.to_lowercase().contains(&query))
                .map(|line| line.text.clone())
                .collect(),
        );

        rect()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new_all(12.))
            .background(colors::page())
            .child(
                LogViewer::new("Launcher logs", lines.clone())
                    .streaming(true)
                    .header(
                        rect()
                            .vertical()
                            .width(Size::fill())
                            .child(view_row(search, severity, lines.len(), entries))
                            .child(crate::ui::divider())
                            .child(filter_row(directives, directives_error))
                            .child(crate::ui::divider()),
                    ),
            )
    }
}

fn push(entries: &mut State<VecDeque<ConsoleLine>>, line: ConsoleLine) {
    let mut entries = entries.write();
    while entries.len() >= MAX_LINES {
        entries.pop_front();
    }
    entries.push_back(line);
}

/// What of the captured lines to show. Purely local: nothing here changes what
/// the launcher records.
fn view_row(
    search: State<String>,
    severity: State<Severity>,
    shown: usize,
    mut entries: State<VecDeque<ConsoleLine>>,
) -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(10., 14.))
        .child(
            rect().width(Size::flex(1.0)).child(
                TextInput::new(search)
                    .placeholder("Filter the stream...")
                    .leading(
                        Icon::new(IconType::SearchMd)
                            .size(15.)
                            .color(colors::fg_secondary())
                            .into_element(),
                    ),
            ),
        )
        .child(
            label()
                .text(format!("{shown} lines"))
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .child(
            SegmentedControl::new(severity)
                .no_tint()
                .segments(
                    [
                        Severity::All,
                        Severity::Error,
                        Severity::Warn,
                        Severity::Info,
                        Severity::Debug,
                    ]
                    .into_iter()
                    .map(|value| Segment::new(value).label(value.label())),
                )
                .into_element(),
        )
        .child(
            Button::new()
                .secondary()
                .on_press(move |_| entries.write().clear())
                .child(Icon::new(IconType::Trash01).size(15.))
                .text("Clear"),
        )
        .into_element()
}

/// The `EnvFilter` directives the whole launcher is running under.
///
/// This is the half that matters: the view filter above can only narrow what
/// was already captured, whereas applying here reloads the subscriber's filter,
/// so a target the launcher was not recording at all starts arriving.
fn filter_row(directives: State<String>, mut error: State<Option<String>>) -> impl IntoElement {
    let apply = move |_| match logger::set_filter(&directives.read()) {
        Ok(()) => error.set(None),
        Err(err) => error.set(Some(err.to_string())),
    };

    let mut reset = directives;
    let row = rect()
        .horizontal()
        .width(Size::fill())
        .cross_align(Alignment::Center)
        .content(Content::Flex)
        .spacing(10.)
        .padding(Gaps::new_symmetric(10., 14.))
        .child(
            label()
                .text("Capture")
                .font_size(11.)
                .max_lines(1)
                .color(colors::fg_secondary()),
        )
        .child(
            rect()
                .width(Size::flex(1.0))
                .child(TextInput::new(directives).placeholder("RUST_LOG directives...")),
        )
        .child(
            Button::new()
                .secondary()
                .on_press(move |_| reset.set(logger::default_directives()))
                .text("Defaults"),
        )
        .child(Button::new().primary().on_press(apply).text("Apply"));

    rect()
        .vertical()
        .width(Size::fill())
        .child(row)
        .maybe_child(error.read().clone().map(|message| {
            label()
                .text(message)
                .font_size(11.)
                .max_lines(2)
                .width(Size::fill())
                .padding(Gaps::new(0., 14., 10., 14.))
                .color(colors::code_error())
                .into_element()
        }))
        .into_element()
}
