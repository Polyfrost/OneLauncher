//! Reconstructing a game session from its log.
//!
//! When the launcher is closed while the game keeps playing, nothing observes
//! the exit. The log is the only witness left, so on the next start we replay
//! it to recover when the session ended and which servers it visited.
//!
//! Minecraft logs a bare wall-clock time (`[12:29:00]`) in the machine's local
//! zone — no date, no offset. Absolute timestamps therefore have to be rebuilt
//! by anchoring at the session start and walking forward.

use chrono::{DateTime, Local, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

use super::session::{ServerJoin, parse_server_join};

/// A backwards jump larger than this means the clock wrapped past midnight
/// rather than lines merely arriving out of order between threads.
const ROLLOVER_SLACK_SECS: i64 = 12 * 60 * 60;

/// Lines that mean "no longer connected to the server we were tracking".
/// Absent any of these the span is closed by the next join, or by the session
/// end — a missing marker costs accuracy, never correctness.
const LEAVE_MARKERS: &[&str] = &[
	"Stopping!",
	"Stopping worker threads",
	"Connection lost",
	"Lost connection to server",
	"Client disconnected with reason",
	"Disconnected from server",
];

/// The clean-shutdown marker. Its timestamp is the truest exit time available.
const STOP_MARKER: &str = "Stopping!";

pub(crate) fn parse_log_time(line: &str) -> Option<NaiveTime> {
	let stamp = line.strip_prefix('[')?.split(']').next()?.trim();

	// `%.f` also matches an empty fraction, so this covers `[12:29:00]` and
	// `[12:29:00.123]` alike.
	if let Ok(time) = NaiveTime::parse_from_str(stamp, "%H:%M:%S%.f") {
		return Some(time);
	}

	// Some pack-shipped log4j configs prepend the date.
	for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%d.%m.%Y %H:%M:%S%.f"] {
		if let Ok(dt) = NaiveDateTime::parse_from_str(stamp, fmt) {
			return Some(dt.time());
		}
	}

	None
}

pub(crate) fn is_leave_marker(line: &str) -> bool {
	LEAVE_MARKERS.iter().any(|marker| line.contains(marker))
}

fn local_to_utc(date: NaiveDate, time: NaiveTime) -> DateTime<Utc> {
	let naive = NaiveDateTime::new(date, time);
	match Local.from_local_datetime(&naive) {
		LocalResult::Single(dt) => dt.with_timezone(&Utc),
		// DST end: the same wall-clock time happens twice. The first is the
		// better guess for a forward-walking log.
		LocalResult::Ambiguous(first, _) => first.with_timezone(&Utc),
		// DST start: this wall-clock time never existed, so step over the gap
		// rather than reading the local time as if it were UTC — that would be
		// wrong by the zone's whole offset.
		LocalResult::None => Local
			.from_local_datetime(&(naive + chrono::Duration::hours(1)))
			.earliest()
			.map(|dt| dt.with_timezone(&Utc))
			.unwrap_or_else(|| Utc.from_utc_datetime(&naive)),
	}
}

/// Turns the log's bare times into absolute instants by walking forward from an
/// anchor and advancing the date whenever the clock wraps.
pub(crate) struct LogClock {
	date: NaiveDate,
	/// High-water mark, not the previous line: threads interleave, so a line
	/// may legitimately sit a second behind the one before it.
	peak: NaiveTime,
}

impl LogClock {
	pub(crate) fn new(anchor: DateTime<Utc>) -> Self {
		let local = anchor.with_timezone(&Local).naive_local();
		Self {
			date: local.date(),
			peak: local.time(),
		}
	}

	pub(crate) fn absolute(&mut self, time: NaiveTime) -> DateTime<Utc> {
		let behind = self.peak.signed_duration_since(time).num_seconds();
		if behind > ROLLOVER_SLACK_SECS {
			self.date = self.date.succ_opt().unwrap_or(self.date);
			self.peak = time;
		} else if time > self.peak {
			self.peak = time;
		}
		local_to_utc(self.date, time)
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerSpan {
	pub host: String,
	pub port: Option<u16>,
	pub joined_at: DateTime<Utc>,
	pub disconnected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct LogReplay {
	/// Timestamp of the last line carrying one — a lower bound on the exit.
	pub last_activity: Option<DateTime<Utc>>,
	/// Set when the game logged a clean shutdown.
	pub stopped_at: Option<DateTime<Utc>>,
	pub servers: Vec<ServerSpan>,
}

/// Walk a whole session log, recovering server spans and the last sign of life.
pub(crate) fn replay(content: &str, started_at: DateTime<Utc>) -> LogReplay {
	let mut clock = LogClock::new(started_at);
	let mut out = LogReplay::default();
	// Timestamps only appear on the first line of a multi-line entry (stack
	// traces continue underneath), so carry the last one forward.
	let mut now = started_at;

	for line in content.lines() {
		if let Some(time) = parse_log_time(line) {
			now = clock.absolute(time);
			out.last_activity = Some(now);
		}

		if let Some(ServerJoin { host, port }) = parse_server_join(line) {
			close_open_span(&mut out, now);
			out.servers.push(ServerSpan {
				host,
				port,
				joined_at: now,
				disconnected_at: None,
			});
			continue;
		}

		if is_leave_marker(line) {
			close_open_span(&mut out, now);
		}

		if line.contains(STOP_MARKER) {
			out.stopped_at = Some(now);
		}
	}

	out
}

fn close_open_span(replay: &mut LogReplay, at: DateTime<Utc>) {
	if let Some(open) = replay
		.servers
		.iter_mut()
		.rev()
		.find(|span| span.disconnected_at.is_none())
	{
		open.disconnected_at = Some(at.max(open.joined_at));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Duration;

	fn anchor() -> DateTime<Utc> {
		Local
			.with_ymd_and_hms(2026, 7, 16, 12, 0, 0)
			.unwrap()
			.with_timezone(&Utc)
	}

	fn local_at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
		Local
			.with_ymd_and_hms(2026, 7, 16, h, m, s)
			.unwrap()
			.with_timezone(&Utc)
	}

	#[test]
	fn parses_plain_and_fractional_times() {
		assert_eq!(
			parse_log_time("[12:29:00] [Client thread/INFO]: Setting user: Lynith"),
			NaiveTime::from_hms_opt(12, 29, 0)
		);
		assert_eq!(
			parse_log_time("[12:29:00.123] [Render thread/INFO]: hi"),
			NaiveTime::from_hms_milli_opt(12, 29, 0, 123)
		);
	}

	#[test]
	fn parses_dated_stamp() {
		assert_eq!(
			parse_log_time("[2026-07-16 12:29:00] [Render thread/INFO]: hi"),
			NaiveTime::from_hms_opt(12, 29, 0)
		);
	}

	#[test]
	fn ignores_untimestamped_lines() {
		assert!(parse_log_time("\tat net.minecraft.Foo.bar(Foo.java:12)").is_none());
		assert!(parse_log_time("[Render thread/INFO]: no time here").is_none());
	}

	#[test]
	fn clock_keeps_same_day_while_moving_forward() {
		let mut clock = LogClock::new(anchor());
		let at = clock.absolute(NaiveTime::from_hms_opt(12, 30, 0).unwrap());
		assert_eq!(at, local_at(12, 30, 0));
	}

	#[test]
	fn clock_rolls_over_midnight() {
		let mut clock = LogClock::new(anchor());
		clock.absolute(NaiveTime::from_hms_opt(23, 59, 0).unwrap());
		let at = clock.absolute(NaiveTime::from_hms_opt(0, 1, 0).unwrap());
		assert_eq!(at, local_at(12, 0, 0) + Duration::hours(12) + Duration::minutes(1));
	}

	#[test]
	fn clock_tolerates_out_of_order_threads() {
		let mut clock = LogClock::new(anchor());
		clock.absolute(NaiveTime::from_hms_opt(12, 29, 8).unwrap());
		// A second thread's line lands a second late; this must not add a day.
		let at = clock.absolute(NaiveTime::from_hms_opt(12, 29, 7).unwrap());
		assert_eq!(at, local_at(12, 29, 7));
	}

	#[test]
	fn replays_server_span_closed_by_quit() {
		let log = "\
[12:29:00] [Client thread/INFO]: Setting user: Lynith
[12:30:00] [Render thread/INFO]: Connecting to mc.hypixel.net, 25565
[12:45:00] [Client thread/INFO]: Stopping!
";
		let out = replay(log, anchor());
		assert_eq!(out.servers.len(), 1);
		let span = &out.servers[0];
		assert_eq!(span.host, "mc.hypixel.net");
		assert_eq!(span.port, Some(25565));
		assert_eq!(span.joined_at, local_at(12, 30, 0));
		assert_eq!(span.disconnected_at, Some(local_at(12, 45, 0)));
		assert_eq!(out.stopped_at, Some(local_at(12, 45, 0)));
		assert_eq!(out.last_activity, Some(local_at(12, 45, 0)));
	}

	#[test]
	fn consecutive_joins_close_previous_span() {
		let log = "\
[12:30:00] [Render thread/INFO]: Connecting to a.example.com, 25565
[12:40:00] [Render thread/INFO]: Connecting to b.example.com, 25566
";
		let out = replay(log, anchor());
		assert_eq!(out.servers.len(), 2);
		assert_eq!(out.servers[0].disconnected_at, Some(local_at(12, 40, 0)));
		assert_eq!(out.servers[1].disconnected_at, None);
	}

	#[test]
	fn span_left_open_when_log_just_stops() {
		let log = "[12:30:00] [Render thread/INFO]: Connecting to a.example.com, 25565\n";
		let out = replay(log, anchor());
		assert_eq!(out.servers[0].disconnected_at, None);
		assert_eq!(out.stopped_at, None);
		assert_eq!(out.last_activity, Some(local_at(12, 30, 0)));
	}

	#[test]
	fn crash_without_stop_marker_reports_last_activity() {
		let log = "\
[12:30:00] [Render thread/INFO]: Connecting to a.example.com, 25565
[12:31:00] [Render thread/ERROR]: Encountered an unexpected exception
\tat net.minecraft.Foo.bar(Foo.java:12)
";
		let out = replay(log, anchor());
		assert_eq!(out.stopped_at, None);
		assert_eq!(out.last_activity, Some(local_at(12, 31, 0)));
	}

	#[test]
	fn empty_log_yields_nothing() {
		let out = replay("", anchor());
		assert!(out.servers.is_empty());
		assert!(out.last_activity.is_none());
		assert!(out.stopped_at.is_none());
	}
}
