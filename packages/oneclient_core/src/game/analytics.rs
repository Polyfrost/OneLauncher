use std::collections::HashMap;
use std::net::IpAddr;

use chrono::{DateTime, Datelike, Local, Timelike};
use oneclient_db::dao::game_session as session_dao;
use oneclient_db::models::{GameSessionServerRow, SessionSpan};

use crate::error::LauncherResult;
use crate::state::LauncherState;

const NIGHT_HOURS: [usize; 8] = [22, 23, 0, 1, 2, 3, 4, 5];
const NIGHT_OWL_SHARE: f64 = 0.35;
const GAMER_SECS_PER_DAY: f64 = 5.0 * 3600.0;

#[derive(Debug, Clone, PartialEq)]
pub struct DayPlaytime {
	pub date: String,
	pub secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persona {
	NightOwl,
	Gamer,
}

impl Persona {
	pub fn title(self) -> &'static str {
		match self {
			Persona::NightOwl => "Night Owl",
			Persona::Gamer => "Gamer",
		}
	}

	pub fn description(self) -> &'static str {
		match self {
			Persona::NightOwl => "Most of your playtime happens after dark.",
			Persona::Gamer => "You average over 5 hours on the days you play.",
		}
	}
}

pub const WEEKDAY_LABELS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Debug, Clone, PartialEq)]
pub struct PlaytimeStats {
	pub total_secs: i64,
	pub session_count: usize,
	pub per_weekday: [i64; 7],
	pub per_hour: [i64; 24],
	pub daily: Vec<DayPlaytime>,
	pub session_secs: Vec<i64>,
	pub active_days: usize,
	pub avg_secs_per_active_day: f64,
	pub peak_hour: Option<usize>,
	pub peak_weekday: Option<usize>,
	pub night_share: f64,
	pub personas: Vec<Persona>,
}

impl PlaytimeStats {
	pub fn from_spans(spans: &[SessionSpan]) -> Self {
		let mut per_weekday = [0i64; 7];
		let mut per_hour = [0i64; 24];
		let mut total_secs = 0i64;
		let mut night_secs = 0i64;
		let mut session_count = 0usize;
		let mut daily: Vec<DayPlaytime> = Vec::new();
		let mut session_secs: Vec<i64> = Vec::new();

		for span in spans {
			let Some(ended) = span.ended_at.as_deref() else {
				continue;
			};
			let (Ok(start), Ok(end)) = (
				DateTime::parse_from_rfc3339(&span.started_at),
				DateTime::parse_from_rfc3339(ended),
			) else {
				continue;
			};

			let secs = (end - start).num_seconds();
			if secs <= 0 {
				continue;
			}

			let local = start.with_timezone(&Local);
			let hour = local.hour() as usize;
			let weekday = local.weekday().num_days_from_monday() as usize;

			total_secs += secs;
			session_count += 1;
			session_secs.push(secs);
			per_hour[hour] += secs;
			per_weekday[weekday] += secs;
			if NIGHT_HOURS.contains(&hour) {
				night_secs += secs;
			}

			let date = local.format("%Y-%m-%d").to_string();
			match daily.last_mut() {
				Some(last) if last.date == date => last.secs += secs,
				_ => daily.push(DayPlaytime { date, secs }),
			}
		}

		let active_days = daily.len();
		let avg_secs_per_active_day = if active_days > 0 {
			total_secs as f64 / active_days as f64
		} else {
			0.0
		};
		let night_share = if total_secs > 0 {
			night_secs as f64 / total_secs as f64
		} else {
			0.0
		};

		let peak_hour = argmax(&per_hour);
		let peak_weekday = argmax(&per_weekday);

		let mut personas = Vec::new();
		if total_secs > 0 && night_share >= NIGHT_OWL_SHARE {
			personas.push(Persona::NightOwl);
		}
		if avg_secs_per_active_day > GAMER_SECS_PER_DAY {
			personas.push(Persona::Gamer);
		}

		Self {
			total_secs,
			session_count,
			per_weekday,
			per_hour,
			daily,
			session_secs,
			active_days,
			avg_secs_per_active_day,
			peak_hour,
			peak_weekday,
			night_share,
			personas,
		}
	}
}

fn argmax(buckets: &[i64]) -> Option<usize> {
	buckets
		.iter()
		.enumerate()
		.filter(|&(_, &v)| v > 0)
		.max_by_key(|&(_, &v)| v)
		.map(|(i, _)| i)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStat {
	pub address: String,
	pub port: Option<i64>,
	pub joins: i64,
	pub total_secs: i64,
	pub last_played: Option<String>,
	pub is_ip: bool,
}

fn address_is_ip(address: &str) -> bool {
	address.parse::<IpAddr>().is_ok()
}

pub fn aggregate_servers(rows: &[GameSessionServerRow]) -> Vec<ServerStat> {
	struct Acc {
		port: Option<i64>,
		joins: i64,
		total_secs: i64,
		last_played: Option<String>,
	}

	let mut map: HashMap<String, Acc> = HashMap::new();
	for row in rows {
		let acc = map.entry(row.address.clone()).or_insert(Acc {
			port: row.port,
			joins: 0,
			total_secs: 0,
			last_played: None,
		});
		acc.joins += 1;

		if let Some(ended) = row.disconnected_at.as_deref()
			&& let (Ok(start), Ok(end)) = (
				DateTime::parse_from_rfc3339(&row.joined_at),
				DateTime::parse_from_rfc3339(ended),
			) {
			let secs = (end - start).num_seconds();
			if secs > 0 {
				acc.total_secs += secs;
			}
		}

		if acc
			.last_played
			.as_deref()
			.is_none_or(|prev| row.joined_at.as_str() > prev)
		{
			acc.last_played = Some(row.joined_at.clone());
			acc.port = row.port;
		}
	}

	let mut servers: Vec<ServerStat> = map
		.into_iter()
		.map(|(address, acc)| ServerStat {
			is_ip: address_is_ip(&address),
			address,
			port: acc.port,
			joins: acc.joins,
			total_secs: acc.total_secs,
			last_played: acc.last_played,
		})
		.collect();

	servers.sort_by(|a, b| {
		b.total_secs
			.cmp(&a.total_secs)
			.then(b.joins.cmp(&a.joins))
			.then(a.address.cmp(&b.address))
	});
	servers
}

#[derive(Debug, Clone, PartialEq)]
pub struct Analytics {
	pub playtime: PlaytimeStats,
	pub servers: Vec<ServerStat>,
}

pub async fn global_analytics() -> LauncherResult<Analytics> {
	let state = LauncherState::get()?;
	let db = &state.services.db;
	let spans = session_dao::all_session_spans(db).await?;
	let servers = session_dao::all_session_servers(db).await?;
	Ok(Analytics {
		playtime: PlaytimeStats::from_spans(&spans),
		servers: aggregate_servers(&servers),
	})
}

pub async fn cluster_analytics(cluster_id: i64) -> LauncherResult<Analytics> {
	let state = LauncherState::get()?;
	let db = &state.services.db;
	let spans = session_dao::session_spans_for_cluster(db, cluster_id).await?;
	let servers = session_dao::session_servers_for_cluster(db, cluster_id).await?;
	Ok(Analytics {
		playtime: PlaytimeStats::from_spans(&spans),
		servers: aggregate_servers(&servers),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	fn span(start: &str, end: &str) -> SessionSpan {
		SessionSpan {
			started_at: start.to_string(),
			ended_at: Some(end.to_string()),
		}
	}

	#[test]
	fn empty_has_no_personas() {
		let stats = PlaytimeStats::from_spans(&[]);
		assert_eq!(stats.total_secs, 0);
		assert!(stats.personas.is_empty());
		assert_eq!(stats.peak_hour, None);
	}

	#[test]
	fn skips_open_and_negative_spans() {
		let spans = vec![
			SessionSpan {
				started_at: "2026-01-01T10:00:00+00:00".into(),
				ended_at: None,
			},
			span("2026-01-01T10:00:00+00:00", "2026-01-01T09:00:00+00:00"),
		];
		let stats = PlaytimeStats::from_spans(&spans);
		assert_eq!(stats.session_count, 0);
		assert_eq!(stats.total_secs, 0);
	}

	fn server(address: &str, joined: &str, disconnected: Option<&str>) -> GameSessionServerRow {
		GameSessionServerRow {
			session_started_at: "s".into(),
			address: address.into(),
			port: Some(25565),
			joined_at: joined.into(),
			disconnected_at: disconnected.map(str::to_string),
		}
	}

	#[test]
	fn ip_addresses_are_flagged_domains_are_not() {
		assert!(address_is_ip("192.168.1.10"));
		assert!(address_is_ip("::1"));
		assert!(!address_is_ip("mc.hypixel.net"));
		assert!(!address_is_ip("play.example.com"));
	}

	#[test]
	fn aggregate_servers_sums_time_and_counts_joins() {
		let rows = vec![
			server(
				"mc.hypixel.net",
				"2026-01-01T10:00:00+00:00",
				Some("2026-01-01T11:00:00+00:00"),
			),
			server("mc.hypixel.net", "2026-01-02T10:00:00+00:00", None),
			server(
				"192.168.1.10",
				"2026-01-01T12:00:00+00:00",
				Some("2026-01-01T12:30:00+00:00"),
			),
		];

		let servers = aggregate_servers(&rows);
		assert_eq!(servers.len(), 2);
		assert_eq!(servers[0].address, "mc.hypixel.net");
		assert_eq!(servers[0].joins, 2);
		assert_eq!(servers[0].total_secs, 3600);
		assert!(!servers[0].is_ip);
		assert_eq!(servers[1].address, "192.168.1.10");
		assert_eq!(servers[1].joins, 1);
		assert_eq!(servers[1].total_secs, 1800);
		assert!(servers[1].is_ip);
	}

	#[test]
	fn gamer_needs_five_hour_average() {
		let spans = vec![span("2026-01-01T12:00:00+00:00", "2026-01-01T18:00:00+00:00")];
		let stats = PlaytimeStats::from_spans(&spans);
		assert_eq!(stats.active_days, 1);
		assert!(stats.personas.contains(&Persona::Gamer));
	}
}
