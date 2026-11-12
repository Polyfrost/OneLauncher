use std::sync::Arc;
use std::time::Duration;

use oneclient_core::LauncherState;
use oneclient_core::game::{Analytics, WEEKDAY_LABELS};

use super::find_cluster;
use crate::Output;
use crate::output::{accent, bold, dim, duration, heading, table};

const RECENT_DAYS: usize = 14;
const LISTED_SERVERS: usize = 10;
const LISTED_CLUSTERS: usize = 10;
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub(super) async fn run(
    state: &Arc<LauncherState>,
    cluster: Option<String>,
    out: &Output,
) -> anyhow::Result<()> {
    let db = &state.services.db;
    let (title, analytics) = match &cluster {
        Some(query) => {
            let cluster = find_cluster(state, query).await?;
            let stats = oneclient_core::game::cluster_analytics(db, cluster.id).await?;
            (cluster.name, stats)
        }
        None => (
            "All clusters".to_string(),
            oneclient_core::game::global_analytics(db).await?,
        ),
    };

    let mut lines = vec![heading(title), String::new()];
    lines.extend(playtime(&analytics));

    if !analytics.personas.is_empty() {
        lines.push(String::new());
        lines.push(bold("Badges"));
        for persona in &analytics.personas {
            lines.push(format!(
                "  {} {}",
                accent(format!("{:<16}", persona.title())),
                dim(persona.description())
            ));
        }
    }

    if !analytics.servers.is_empty() {
        lines.push(String::new());
        lines.push(bold("Servers"));
        let rows: Vec<Vec<String>> = analytics
            .servers
            .iter()
            .take(LISTED_SERVERS)
            .map(|server| {
                vec![
                    match server.port {
                        Some(port) => format!("{}:{port}", server.address),
                        None => server.address.clone(),
                    },
                    server.joins.to_string(),
                    secs(server.total_secs),
                    server.last_played.clone().unwrap_or_default(),
                ]
            })
            .collect();
        lines.push(table(&["ADDRESS", "JOINS", "PLAYTIME", "LAST"], &rows));
    }

    if cluster.is_none() {
        let mut clusters = state.clusters.list().await?;
        clusters.retain(|cluster| !cluster.overall_played.is_zero());
        clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.overall_played));
        if !clusters.is_empty() {
            lines.push(String::new());
            lines.push(bold("Most played"));
            let rows: Vec<Vec<String>> = clusters
                .iter()
                .take(LISTED_CLUSTERS)
                .map(|cluster| vec![cluster.name.clone(), duration(cluster.overall_played)])
                .collect();
            lines.push(table(&["CLUSTER", "PLAYTIME"], &rows));
        }
    }

    out.line(lines.join("\n"));
    Ok(())
}

fn secs(secs: i64) -> String {
    duration(Duration::from_secs(secs.max(0) as u64))
}

fn bar(value: i64, max: i64) -> char {
    if value <= 0 || max <= 0 {
        return ' ';
    }
    let step = (value * (BARS.len() as i64 - 1)) / max;
    BARS[step as usize]
}

fn playtime(analytics: &Analytics) -> Vec<String> {
    let stats = &analytics.playtime;
    if stats.session_count == 0 {
        return vec![dim("No play sessions yet")];
    }

    let field = |name: &str, value: String| format!("  {} {value}", dim(format!("{name:<18}")));
    let mut lines = vec![
        field("Total playtime", secs(stats.total_secs)),
        field("Sessions", stats.session_count.to_string()),
        field("Longest session", secs(stats.longest_session_secs)),
        field("Active days", stats.active_days.to_string()),
        field(
            "Streak",
            format!(
                "{} days, best {}",
                stats.current_streak, stats.longest_streak
            ),
        ),
        field("Per active day", secs(stats.avg_secs_per_active_day as i64)),
        field("After dark", format!("{:.0}%", stats.night_share * 100.0)),
    ];
    if let Some(hour) = stats.peak_hour {
        lines.push(field("Busiest hour", format!("{hour:02}:00")));
    }
    if let Some(day) = stats.peak_weekday {
        lines.push(field("Busiest day", WEEKDAY_LABELS[day].to_string()));
    }

    let recent: Vec<_> = stats.daily.iter().rev().take(RECENT_DAYS).rev().collect();
    if !recent.is_empty() {
        let max = recent.iter().map(|day| day.secs).max().unwrap_or(0);
        let chart: String = recent.iter().map(|day| bar(day.secs, max)).collect();
        lines.push(field(&format!("Last {} days", recent.len()), accent(chart)));
    }

    let max = stats.per_weekday.iter().copied().max().unwrap_or(0);
    let week: String = stats
        .per_weekday
        .iter()
        .zip(WEEKDAY_LABELS)
        .map(|(secs, label)| format!("{label} {}", bar(*secs, max)))
        .collect::<Vec<_>>()
        .join("  ");
    lines.push(field("By weekday", week));
    lines
}
