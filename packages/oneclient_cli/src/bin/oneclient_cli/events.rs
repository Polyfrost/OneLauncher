use std::collections::HashMap;
use std::io::{BufRead, IsTerminal, Write};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use oneclient_events::{
    Answer, ChoiceInput, CrashModAction, CrashModIssueKind, CrashRemedy, Event, EventReceiver,
    GameCrash, GameEvent, GroupedProgressEvent, Level, Notification, ProgressEvent, PromptRequest,
};
use uuid::Uuid;

struct Group {
    parent: ProgressBar,
    children: HashMap<Uuid, (ProgressBar, u64, u64)>,
}

impl Group {
    fn refresh(&self) {
        let (current, total) = self
            .children
            .values()
            .fold((0, 0), |(c, t), (_, cur, tot)| (c + cur, t + tot));
        self.parent.set_length(total.max(1));
        self.parent.set_position(current.min(total.max(1)));
    }
}

pub fn spawn(mut events: EventReceiver) {
    tokio::spawn(async move {
        let bars = MultiProgress::new();
        let flat = ProgressStyle::with_template("{msg:<32} [{bar:30.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("=> ");
        let parent = ProgressStyle::with_template(
            "{spinner:.green} {msg:<30} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("=> ");
        let child =
            ProgressStyle::with_template("    {prefix:<12} {msg:<30} {bytes}/{total_bytes}")
                .unwrap();

        let mut flats: HashMap<Uuid, ProgressBar> = HashMap::new();
        let mut groups: HashMap<Uuid, Group> = HashMap::new();
        let say = |text: String| bars.suspend(|| eprintln!("{text}"));

        while let Some(event) = events.recv().await {
            match event {
                Event::Notification(Notification::Message(message)) => {
                    let title = match message.level {
                        Level::Info => console::style(message.title).bold(),
                        Level::Error => console::style(message.title).bold().red(),
                    };
                    let link = message
                        .link
                        .map(|link| format!(" {}", console::style(link).cyan()))
                        .unwrap_or_default();
                    match message.body.is_empty() {
                        true => say(format!("{title}{link}")),
                        false => say(format!("{title}: {}{link}", message.body)),
                    }
                }
                Event::Notification(Notification::Prompt(request)) => {
                    let answer = tokio::task::block_in_place(|| bars.suspend(|| ask(&request)));
                    let _ = request.reply.send(answer);
                }
                Event::Progress(ProgressEvent::Update {
                    id,
                    label,
                    current,
                    total,
                }) => {
                    if current >= total {
                        if let Some(bar) = flats.remove(&id) {
                            bar.finish_and_clear();
                        }
                        continue;
                    }
                    let bar = flats.entry(id).or_insert_with(|| {
                        let bar = bars.add(ProgressBar::new(total));
                        bar.set_style(flat.clone());
                        bar
                    });
                    bar.set_message(label);
                    bar.set_length(total);
                    bar.set_position(current);
                }
                Event::Progress(ProgressEvent::Complete {
                    id,
                    title,
                    body,
                    level,
                }) => {
                    if let Some(bar) = flats.remove(&id) {
                        bar.finish_and_clear();
                    }
                    let title = match level {
                        Level::Info => console::style(title).bold(),
                        Level::Error => console::style(title).bold().red(),
                    };
                    say(format!("{title}: {body}"));
                }
                Event::Progress(ProgressEvent::Grouped(event)) => match event {
                    GroupedProgressEvent::Start { session_id, title } => {
                        let bar = bars.add(ProgressBar::new(1));
                        bar.set_style(parent.clone());
                        bar.set_message(title);
                        groups.insert(
                            session_id,
                            Group {
                                parent: bar,
                                children: HashMap::new(),
                            },
                        );
                    }
                    GroupedProgressEvent::AddChild {
                        session_id,
                        child_id,
                        label,
                        total,
                        ..
                    } => {
                        if let Some(group) = groups.get_mut(&session_id) {
                            let bar = bars.insert_after(&group.parent, ProgressBar::new(total));
                            bar.set_style(child.clone());
                            bar.set_message(label);
                            group.children.insert(child_id, (bar, 0, total));
                            group.refresh();
                        }
                    }
                    GroupedProgressEvent::UpdateChild {
                        session_id,
                        child_id,
                        current,
                        total,
                    } => {
                        if let Some(group) = groups.get_mut(&session_id) {
                            if let Some((bar, cur, tot)) = group.children.get_mut(&child_id) {
                                bar.set_length(total);
                                bar.set_position(current);
                                (*cur, *tot) = (current, total);
                            }
                            group.refresh();
                        }
                    }
                    GroupedProgressEvent::SetChildPhase {
                        session_id,
                        child_id,
                        phase,
                    } => {
                        if let Some((bar, ..)) = groups
                            .get(&session_id)
                            .and_then(|group| group.children.get(&child_id))
                        {
                            bar.set_prefix(phase.label());
                        }
                    }
                    GroupedProgressEvent::FinishChild {
                        session_id,
                        child_id,
                    } => {
                        if let Some(group) = groups.get_mut(&session_id) {
                            if let Some((bar, ..)) = group.children.remove(&child_id) {
                                bar.finish_and_clear();
                            }
                            group.refresh();
                        }
                    }
                    GroupedProgressEvent::End { session_id }
                    | GroupedProgressEvent::Abort { session_id } => {
                        if let Some(group) = groups.remove(&session_id) {
                            for (bar, ..) in group.children.into_values() {
                                bar.finish_and_clear();
                            }
                            group.parent.finish_and_clear();
                        }
                    }
                    GroupedProgressEvent::Expect { .. } => {}
                },
                Event::Game(GameEvent::Failed { message, .. }) => {
                    say(format!("{} {message}", console::style("error:").red()));
                }
                Event::Game(GameEvent::Crashed(crash)) => say(crash_report(&crash)),
                Event::Game(_) | Event::Signal(_) | Event::Chat(_) => {}
            }
        }
    });
}

fn crash_report(crash: &GameCrash) -> String {
    let id = crash.cluster_id;
    let mut lines = vec![
        String::new(),
        console::style(&crash.title).bold().red().to_string(),
    ];
    lines.extend(crash.cause.clone());
    lines.push(
        console::style(format!(
            "Exited with {} after {}m {}s",
            crash.exit,
            crash.played_secs / 60,
            crash.played_secs % 60
        ))
        .dim()
        .to_string(),
    );
    if !crash.suspects.is_empty() {
        lines.push(format!("Suspected mods: {}", crash.suspects.join(", ")));
    }

    let mut fixes = Vec::new();
    for issue in &crash.mod_issues {
        let kind = match issue.kind {
            CrashModIssueKind::Missing => "Missing",
            CrashModIssueKind::WrongVersion => "Wrong version",
            CrashModIssueKind::Incompatible => "Incompatible",
        };
        lines.push(format!(
            "{kind}: {} {}",
            console::style(&issue.target.name).bold(),
            console::style(&issue.target.note).dim()
        ));
        for source in &issue.sources {
            lines.push(format!(
                "    {} {}",
                source.name,
                console::style(&source.note).dim()
            ));
        }
        for row in std::iter::once(&issue.target).chain(&issue.sources) {
            fixes.extend(row.action.as_ref().map(|action| match action {
                CrashModAction::Enable { hash } => {
                    format!("oneclient_cli package enable {id} {hash}")
                }
                CrashModAction::Install { mod_id, .. } => {
                    format!("oneclient_cli package install {id} {mod_id} --mod-id")
                }
            }));
        }
    }

    let game_dir = crash.game_dir.as_deref().unwrap_or("your game folder");
    for fix in &crash.fixes {
        let text = fix
            .text
            .replace("%profileroot%", game_dir)
            .replace("%gameroot%", game_dir)
            .replace("%pathindicator%", "`");
        lines.push(format!("Fix: {text}"));
    }

    if !crash.excerpt.is_empty() {
        lines.push(console::style("Last lines of the log:").dim().to_string());
        for line in &crash.excerpt {
            lines.push(format!("    {}", strip_formatting(line)));
        }
    }

    fixes.extend(crash.remedy.map(|remedy| match remedy {
        CrashRemedy::VerifyFiles => format!("oneclient_cli cluster repair {id}"),
        CrashRemedy::RaiseMemory => format!("oneclient_cli cluster settings {id}"),
        CrashRemedy::OpenJavaSettings => "oneclient_cli java list".to_string(),
        CrashRemedy::OpenMods => format!("oneclient_cli package list {id}"),
    }));
    fixes.push(format!("oneclient_cli logs show {id}"));
    fixes.push(format!("oneclient_cli logs upload {id}"));
    lines.push("Try:".to_string());
    lines.extend(
        fixes
            .into_iter()
            .map(|fix| format!("    {}", console::style(fix).cyan())),
    );

    lines.join("\n")
}

/// Minecraft colours text with a section sign followed by one code character
fn strip_formatting(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

fn ask(request: &PromptRequest) -> Option<Answer> {
    eprintln!("\n{}", console::style(&request.title).bold());
    if !request.body.is_empty() {
        eprintln!("{}", request.body);
    }
    if !std::io::stdin().is_terminal() {
        eprintln!("No terminal to answer on, declining");
        return None;
    }

    for (n, choice) in request.choices.iter().enumerate() {
        eprintln!("  {}) {}", n + 1, choice.label);
    }
    let dismiss = request.dismiss.as_deref().unwrap_or("Cancel");
    eprintln!("  0) {dismiss}");

    let pick = loop {
        let line = read_line("Choose: ")?;
        match line.parse::<usize>() {
            Ok(0) => return None,
            Ok(n) if n <= request.choices.len() => break &request.choices[n - 1],
            _ => eprintln!("Enter a number from 0 to {}", request.choices.len()),
        }
    };

    let answer = Answer::new(pick.id);
    Some(match &pick.input {
        Some(ChoiceInput::Folder { title }) => {
            answer.with_folder(read_line(&format!("{title}: "))?)
        }
        Some(ChoiceInput::Selection { hint }) => {
            match read_line(&format!("{hint} (blank for any): "))? {
                value if value.is_empty() => answer,
                value => answer.with_selection(value),
            }
        }
        None => answer,
    })
}

fn read_line(prompt: &str) -> Option<String> {
    eprint!("{prompt}");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}
