use std::io::{BufRead, IsTerminal, Write};
use std::process::{Command, Stdio};

use oneclient_cli::Reply;

pub fn print_reply(reply: Reply) {
    match reply {
        Reply::Out(text) => println!("{}", for_stdout(&text)),
        Reply::Err(text) => eprintln!("{}", for_stderr(&text)),
        Reply::Page(text) => page(&text),
        Reply::Exit(_) | Reply::Released => {}
    }
}

fn for_stdout(text: &str) -> String {
    if console::colors_enabled() {
        text.to_string()
    } else {
        console::strip_ansi_codes(text).into_owned()
    }
}

fn for_stderr(text: &str) -> String {
    if console::colors_enabled_stderr() {
        text.to_string()
    } else {
        console::strip_ansi_codes(text).into_owned()
    }
}

fn pager() -> (String, Vec<String>) {
    let configured = std::env::var("PAGER")
        .ok()
        .filter(|pager| !pager.trim().is_empty());
    let fallback = if cfg!(windows) { "more" } else { "less" };
    let pager = configured.unwrap_or_else(|| fallback.to_string());

    let mut words = pager.split_whitespace().map(str::to_string);
    let program = words.next().unwrap_or_else(|| fallback.to_string());
    (program, words.collect())
}

fn page(text: &str) {
    let stdout = std::io::stdout();
    if !stdout.is_terminal() {
        println!("{}", for_stdout(text));
        return;
    }

    let (program, args) = pager();
    let mut command = Command::new(&program);
    command.args(&args).stdin(Stdio::piped());
    if std::env::var_os("LESS").is_none() {
        command.env("LESS", "FRX");
    }

    let text = if program.eq_ignore_ascii_case("more") {
        console::strip_ansi_codes(text).into_owned()
    } else {
        for_stdout(text)
    };

    let Ok(mut child) = command.spawn() else {
        println!("{text}");
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
        let _ = stdin.write_all(b"\n");
    }
    let _ = child.wait();
}

pub fn confirm(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        eprintln!("{question} Pass --yes to confirm without a terminal");
        return false;
    }
    eprint!("{question} [y/N] ");
    let _ = std::io::stderr().flush();

    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}
