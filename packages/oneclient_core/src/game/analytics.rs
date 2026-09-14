use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike, Utc};
use oneclient_db::dao::game_session as session_dao;
use oneclient_db::models::{GameSessionServerRow, SessionSpan};

use crate::error::LauncherResult;

const NIGHT_HOURS: [usize; 8] = [22, 23, 0, 1, 2, 3, 4, 5];
const MORNING_HOURS: [usize; 4] = [5, 6, 7, 8];
const WEEKEND_DAYS: [usize; 2] = [5, 6];

const NIGHT_OWL_SHARE: f64 = 0.35;
const EARLY_BIRD_SHARE: f64 = 0.30;
const WEEKEND_SHARE: f64 = 0.55;
const LOYALIST_SHARE: f64 = 0.70;

const GAMER_SECS_PER_DAY: f64 = 5.0 * 3600.0;
const MARATHON_SECS: i64 = 8 * 3600;
const VETERAN_SECS: i64 = 500 * 3600;
const REGULAR_STREAK: usize = 7;
const EXPLORER_SERVERS: usize = 10;
const SPRINTER_SECS: i64 = 30 * 60;
const SPRINTER_SESSIONS: usize = 10;

const MAX_PERSONAS: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct DayPlaytime {
	pub date: String,
	pub secs: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persona {
	Veteran,
	Marathoner,
	Regular,
	Loyalist,
	Explorer,
	NightOwl,
	EarlyBird,
	WeekendWarrior,
	Gamer,
	Sprinter,
}

impl Persona {
	pub fn title(self) -> &'static str {
		match self {
			Persona::Veteran => "Veteran",
			Persona::Marathoner => "Marathoner",
			Persona::Regular => "Regular",
			Persona::Loyalist => "Loyalist",
			Persona::Explorer => "Explorer",
			Persona::NightOwl => "Night Owl",
			Persona::EarlyBird => "Early Bird",
			Persona::WeekendWarrior => "Weekend Warrior",
			Persona::Gamer => "Gamer",
			Persona::Sprinter => "Sprinter",
		}
	}

	pub fn description(self) -> &'static str {
		match self {
			Persona::Veteran => "500h+ played",
			Persona::Marathoner => "An 8h+ session",
			Persona::Regular => "A week straight",
			Persona::Loyalist => "Mostly one server",
			Persona::Explorer => "10+ servers joined",
			Persona::NightOwl => "Mostly after dark",
			Persona::EarlyBird => "Mostly before 9am",
			Persona::WeekendWarrior => "Mostly weekends",
			Persona::Gamer => "5h+ per day played",
			Persona::Sprinter => "Short and frequent",
		}
	}
}

fn personas_for(stats: &PlaytimeStats, servers: &[ServerStat]) -> Vec<Persona> {
	if stats.total_secs <= 0 {
		return Vec::new();
	}

	let total = stats.total_secs as f64;
	let share = |hours: &[usize]| -> f64 {
		hours.iter().map(|&h| stats.per_hour[h]).sum::<i64>() as f64 / total
	};
	let weekend = WEEKEND_DAYS
		.iter()
		.map(|&d| stats.per_weekday[d])
		.sum::<i64>() as f64
		/ total;

	let server_secs: i64 = servers.iter().map(|s| s.total_secs).sum();
	let top_server_share = servers
		.iter()
		.map(|s| s.total_secs)
		.max()
		.filter(|_| server_secs > 0)
		.map_or(0.0, |top| top as f64 / server_secs as f64);

	let mut lengths: Vec<i64> = stats.session_secs.clone();
	lengths.sort_unstable();
	let median = lengths.get(lengths.len() / 2).copied().unwrap_or(0);

	let earned = [
		(Persona::Veteran, stats.total_secs >= VETERAN_SECS),
		(
			Persona::Marathoner,
			stats.longest_session_secs >= MARATHON_SECS,
		),
		(Persona::Regular, stats.longest_streak >= REGULAR_STREAK),
		(
			Persona::Loyalist,
			servers.len() > 1 && top_server_share >= LOYALIST_SHARE,
		),
		(Persona::Explorer, servers.len() >= EXPLORER_SERVERS),
		(Persona::NightOwl, share(&NIGHT_HOURS) >= NIGHT_OWL_SHARE),
		(
			Persona::EarlyBird,
			share(&MORNING_HOURS) >= EARLY_BIRD_SHARE,
		),
		(Persona::WeekendWarrior, weekend >= WEEKEND_SHARE),
		(
			Persona::Gamer,
			stats.avg_secs_per_active_day > GAMER_SECS_PER_DAY,
		),
		(
			Persona::Sprinter,
			stats.session_count >= SPRINTER_SESSIONS && median < SPRINTER_SECS,
		),
	];

	earned
		.into_iter()
		.filter_map(|(persona, earned)| earned.then_some(persona))
		.take(MAX_PERSONAS)
		.collect()
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
	pub longest_session_secs: i64,
	pub active_days: usize,
	pub current_streak: usize,
	pub longest_streak: usize,
	pub avg_secs_per_active_day: f64,
	pub peak_hour: Option<usize>,
	pub peak_weekday: Option<usize>,
	pub night_share: f64,
}

impl PlaytimeStats {
	pub fn from_spans(spans: &[SessionSpan]) -> Self {
		Self::from_spans_on(spans, Local::now().date_naive())
	}

	fn from_spans_on(spans: &[SessionSpan], today: NaiveDate) -> Self {
		let mut per_weekday = [0i64; 7];
		let mut per_hour = [0i64; 24];
		let mut total_secs = 0i64;
		let mut night_secs = 0i64;
		let mut session_count = 0usize;
		let mut by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
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

			total_secs += secs;
			session_count += 1;
			session_secs.push(secs);

			for_each_local_hour(
				start.with_timezone(&Utc),
				end.with_timezone(&Utc),
				|at, slice| {
					let hour = at.hour() as usize;
					per_hour[hour] += slice;
					per_weekday[at.weekday().num_days_from_monday() as usize] += slice;
					if NIGHT_HOURS.contains(&hour) {
						night_secs += slice;
					}
					*by_day.entry(at.date_naive()).or_insert(0) += slice;
				},
			);
		}

		let longest_session_secs = session_secs.iter().copied().max().unwrap_or(0);
		let dates: Vec<NaiveDate> = by_day.keys().copied().collect();
		let (current_streak, longest_streak) = streaks(&dates, today);
		let daily: Vec<DayPlaytime> = by_day
			.iter()
			.map(|(date, secs)| DayPlaytime {
				date: date.format("%Y-%m-%d").to_string(),
				secs: *secs,
			})
			.collect();

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

		Self {
			total_secs,
			session_count,
			per_weekday,
			per_hour,
			daily,
			session_secs,
			longest_session_secs,
			active_days,
			current_streak,
			longest_streak,
			avg_secs_per_active_day,
			peak_hour,
			peak_weekday,
			night_share,
		}
	}
}

fn for_each_local_hour(
	start: DateTime<Utc>,
	end: DateTime<Utc>,
	mut f: impl FnMut(DateTime<Local>, i64),
) {
	let mut cur = start;
	while cur < end {
		let local = cur.with_timezone(&Local);
		let boundary = next_local_hour(local);
		let next = if boundary > cur {
			boundary.min(end)
		} else {
			(cur + Duration::hours(1)).min(end)
		};
		f(local, (next - cur).num_seconds());
		cur = next;
	}
}

fn next_local_hour(local: DateTime<Local>) -> DateTime<Utc> {
	let naive = local.naive_local();
	let top = match naive.date().and_hms_opt(naive.hour(), 0, 0) {
		Some(top) => top + Duration::hours(1),
		None => return (local + Duration::hours(1)).with_timezone(&Utc),
	};

	Local
		.from_local_datetime(&top)
		.earliest()
		.map(|at| at.with_timezone(&Utc))
		.unwrap_or_else(|| (local + Duration::hours(1)).with_timezone(&Utc))
}

fn streaks(days: &[NaiveDate], today: NaiveDate) -> (usize, usize) {
	let mut longest = 0usize;
	let mut run = 0usize;
	let mut prev: Option<NaiveDate> = None;

	for &day in days {
		run = match prev {
			Some(p) if p == day => run,
			Some(p) if p.succ_opt() == Some(day) => run + 1,
			_ => 1,
		};
		longest = longest.max(run);
		prev = Some(day);
	}

	let current = match days.last() {
		Some(last) if *last == today || Some(*last) == today.pred_opt() => run,
		_ => 0,
	};

	(current, longest)
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
	pub personas: Vec<Persona>,
}

impl Analytics {
	pub fn new(playtime: PlaytimeStats, servers: Vec<ServerStat>) -> Self {
		let personas = personas_for(&playtime, &servers);
		Self {
			playtime,
			servers,
			personas,
		}
	}
}

#[tracing::instrument(level = "debug", skip(db))]
pub async fn global_analytics(db: &oneclient_db::DbPool) -> LauncherResult<Analytics> {
	let spans = session_dao::all_session_spans(db).await?;
	let servers = session_dao::all_session_servers(db).await?;
	Ok(Analytics::new(
		PlaytimeStats::from_spans(&spans),
		aggregate_servers(&servers),
	))
}

#[tracing::instrument(level = "debug", skip(db))]
pub async fn cluster_analytics(db: &oneclient_db::DbPool, cluster_id: i64) -> LauncherResult<Analytics> {
	let spans = session_dao::session_spans_for_cluster(db, cluster_id).await?;
	let servers = session_dao::session_servers_for_cluster(db, cluster_id).await?;
	Ok(Analytics::new(
		PlaytimeStats::from_spans(&spans),
		aggregate_servers(&servers),
	))
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
		assert!(personas_for(&stats, &[]).is_empty());
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

	fn day(s: &str) -> NaiveDate {
		NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("a date")
	}

	fn local_span(start: &str, end: &str) -> SessionSpan {
		let at = |s: &str| {
			Local
				.from_local_datetime(
					&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("a time"),
				)
				.earliest()
				.expect("an unambiguous local time")
				.to_rfc3339()
		};
		SessionSpan {
			started_at: at(start),
			ended_at: Some(at(end)),
		}
	}

	#[test]
	fn an_overnight_session_is_split_across_both_days() {
		let spans = vec![local_span("2026-07-16 22:00:00", "2026-07-17 04:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-07-17"));

		assert_eq!(stats.total_secs, 6 * 3600);
		assert_eq!(stats.session_count, 1, "still one session");
		assert_eq!(stats.daily.len(), 2);
		assert_eq!(stats.daily[0].date, "2026-07-16");
		assert_eq!(stats.daily[0].secs, 2 * 3600);
		assert_eq!(stats.daily[1].date, "2026-07-17");
		assert_eq!(stats.daily[1].secs, 4 * 3600);
	}

	#[test]
	fn a_multi_day_session_books_a_full_day_per_day() {
		let spans = vec![local_span("2026-07-16 12:00:00", "2026-07-19 12:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-07-19"));

		assert_eq!(stats.daily.len(), 4, "three nights means four days touched");
		assert_eq!(stats.daily[1].secs, 24 * 3600);
		assert_eq!(stats.daily[2].secs, 24 * 3600);
		assert_eq!(
			stats.daily.iter().map(|d| d.secs).sum::<i64>(),
			stats.total_secs
		);
		assert!(stats.per_hour.iter().all(|&h| h > 0));
	}

	#[test]
	fn weekday_totals_follow_the_hours_played_not_the_start() {
		let spans = vec![local_span("2026-07-17 23:00:00", "2026-07-18 09:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-07-18"));

		assert_eq!(stats.per_weekday[4], 3600, "Friday keeps only its one hour");
		assert_eq!(stats.per_weekday[5], 9 * 3600);
		assert_eq!(stats.peak_weekday, Some(5));
	}

	#[test]
	fn a_streak_survives_an_empty_today_but_not_an_empty_yesterday() {
		let days = [day("2026-07-15"), day("2026-07-16"), day("2026-07-17")];

		assert_eq!(streaks(&days, day("2026-07-17")), (3, 3));
		assert_eq!(streaks(&days, day("2026-07-18")), (3, 3), "today is not over");
		assert_eq!(streaks(&days, day("2026-07-19")), (0, 3), "yesterday was missed");
	}

	#[test]
	fn a_gap_ends_the_run_but_keeps_the_record() {
		let days = [
			day("2026-07-01"),
			day("2026-07-02"),
			day("2026-07-03"),
			day("2026-07-10"),
		];
		assert_eq!(streaks(&days, day("2026-07-10")), (1, 3));
		assert_eq!(streaks(&[], day("2026-07-10")), (0, 0));
	}

	#[test]
	fn gamer_needs_five_hour_average() {
		let spans = vec![local_span("2026-01-01 12:00:00", "2026-01-01 18:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-01-01"));
		assert_eq!(stats.active_days, 1);
		assert!(personas_for(&stats, &[]).contains(&Persona::Gamer));
	}

	fn stat_server(address: &str, total_secs: i64) -> ServerStat {
		ServerStat {
			address: address.into(),
			port: None,
			joins: 1,
			total_secs,
			last_played: None,
			is_ip: false,
		}
	}

	#[test]
	fn loyalty_needs_somewhere_else_to_have_been() {
		let spans = vec![local_span("2026-01-01 12:00:00", "2026-01-01 18:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-01-01"));

		let only_one = [stat_server("a.example.com", 3600)];
		assert!(
			!personas_for(&stats, &only_one).contains(&Persona::Loyalist),
			"a single server is loyalty to nothing"
		);

		let favourite = [
			stat_server("a.example.com", 9000),
			stat_server("b.example.com", 1000),
		];
		assert!(personas_for(&stats, &favourite).contains(&Persona::Loyalist));
	}

	#[test]
	fn an_overnight_session_still_reads_as_a_night_owl() {
		let spans = vec![local_span("2026-01-01 22:00:00", "2026-01-02 06:00:00")];
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-01-02"));

		assert!(personas_for(&stats, &[]).contains(&Persona::NightOwl));
		assert!(personas_for(&stats, &[]).contains(&Persona::Marathoner));
	}

	#[test]
	fn no_more_personas_than_fit() {
		let mut spans = Vec::new();
		for day_of in 1..=20 {
			spans.push(local_span(
				&format!("2026-01-{day_of:02} 22:00:00"),
				&format!("2026-01-{:02} 08:00:00", day_of + 1),
			));
		}
		let stats = PlaytimeStats::from_spans_on(&spans, day("2026-01-21"));
		let servers: Vec<ServerStat> = (0..12)
			.map(|i| stat_server(&format!("s{i}.example.com"), 3600))
			.collect();

		assert_eq!(personas_for(&stats, &servers).len(), MAX_PERSONAS);
	}
}
