//! Async utilities for parsing Log4j entries with [`nom`].
//! From <https://github.com/gorilla-devs/GDLauncher-Carbon/blob/develop/crates/carbon_parsing/src/log.rs>

use nom::branch::alt;
use nom::bytes::streaming::{tag, take_until};
use nom::character::streaming::{char, multispace0, u64};
use nom::combinator::{map, value};
use nom::error::ParseError;
use nom::multi::count;
use nom::sequence::{delimited, preceded, separated_pair, tuple};
use nom::IResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LogEntry<'a> {
	pub logger: &'a str,
	pub level: LogLevel,
	pub timestamp: u64,
	pub thread_name: &'a str,
	pub message: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogLevel {
	Trace,
	Debug,
	Info,
	Warn,
	Error,
}

fn whitespace<'a, O, E: ParseError<&'a str>>(
	inner: impl FnMut(&'a str) -> IResult<&'a str, O, E>,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E> {
	delimited(multispace0, inner, multispace0)
}

pub fn parse_logentry(input: &str) -> IResult<&str, LogEntry<'_>> {
	let (o, (attributes, _, message)) = preceded(
		multispace0,
		alt((
			delimited(
				tag("<log4j:Event"),
				tuple((attributes, tag(">"), whitespace(message))),
				tag("</log4j:Event>"),
			),
			plain_text,
		)),
	)(input)?;

	let Attributes {
		logger,
		level,
		timestamp,
		thread_name,
	} = attributes;

	Ok((
		o,
		LogEntry {
			logger,
			level,
			timestamp,
			thread_name,
			message,
		},
	))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Attributes<'a> {
	pub logger: &'a str,
	pub level: LogLevel,
	pub timestamp: u64,
	pub thread_name: &'a str,
}

fn attributes(input: &str) -> IResult<&str, Attributes<'_>> {
	let (o, attributes) = count(whitespace(attribute), 4)(input)?;
	macro_rules! extract_attribute {
		($field:ident) => {{
			let err = nom::Err::Error(nom::error::Error::from_error_kind(
				o,
				nom::error::ErrorKind::Alt,
			));
			let Attribute::$field(field) = attributes
				.iter()
				.copied()
				.find(|a| matches!(a, Attribute::$field(_)))
				.ok_or(err)?
			else {
				unreachable!();
			};

			field
		}};
	}

	Ok((
		o,
		Attributes {
			logger: extract_attribute!(Logger),
			level: extract_attribute!(Level),
			timestamp: extract_attribute!(Timestamp),
			thread_name: extract_attribute!(ThreadName),
		},
	))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Attribute<'a> {
	Logger(&'a str),
	Level(LogLevel),
	Timestamp(u64),
	ThreadName(&'a str),
}

fn attribute(input: &str) -> IResult<&str, Attribute<'_>> {
	alt((attr_logger, attr_timestamp, attr_level, attr_thread))(input)
}

fn attr_logger(input: &str) -> IResult<&str, Attribute<'_>> {
	map(
		separated_pair(
			tag("logger"),
			whitespace(char('=')),
			map(quoted_string, Attribute::Logger),
		),
		|(_, attr)| attr,
	)(input)
}

fn attr_timestamp(input: &str) -> IResult<&str, Attribute<'_>> {
	map(
		separated_pair(
			tag("timestamp"),
			whitespace(char('=')),
			delimited(char('"'), map(u64, Attribute::Timestamp), char('"')),
		),
		|(_, attr)| attr,
	)(input)
}

fn attr_level(input: &str) -> IResult<&str, Attribute<'_>> {
	map(
		separated_pair(
			tag("level"),
			whitespace(char('=')),
			delimited(char('"'), map(level, Attribute::Level), char('"')),
		),
		|(_, attr)| attr,
	)(input)
}

fn attr_thread(input: &str) -> IResult<&str, Attribute<'_>> {
	map(
		separated_pair(
			tag("thread"),
			whitespace(char('=')),
			map(quoted_string, Attribute::ThreadName),
		),
		|(_, attr)| attr,
	)(input)
}

fn quoted_string(input: &str) -> IResult<&str, &str> {
	delimited(char('"'), take_until("\""), char('"'))(input)
}

fn level(input: &str) -> IResult<&str, LogLevel> {
	alt((
		value(LogLevel::Trace, tag("TRACE")),
		value(LogLevel::Debug, tag("DEBUG")),
		value(LogLevel::Info, tag("INFO")),
		value(LogLevel::Warn, tag("WARN")),
		value(LogLevel::Error, tag("ERROR")),
	))(input)
}

/// Parses the message of the event.
fn message(input: &str) -> IResult<&str, &str> {
	delimited(
		tag("<log4j:Message>"),
		whitespace(delimited(tag("<![CDATA["), take_until("]]>"), tag("]]>"))),
		tag("</log4j:Message>"),
	)(input)
}

#[allow(clippy::cast_sign_loss)]
fn plain_text(input: &str) -> IResult<&str, (Attributes<'_>, &str, &str)> {
	map(take_until("<"), |text| {
		(
			Attributes {
				logger: "OneLauncher",
				level: LogLevel::Info,
				timestamp: chrono::Local::now().timestamp_millis() as u64,
				thread_name: "N/A",
			},
			"",
			text,
		)
	})(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use super::*;

	#[test]
	fn parse_message() {
		message(
			r"<log4j:Message>
                <![CDATA[200 Datafixer optimizations took 2000 years]]>
            </log4j:Message>",
		)
		.unwrap();
	}

	#[test]
	fn parse_attributes() {
		let (_, attributes) = attributes(
			r#"
            logger="com.mojang.datafixers.DataFixerBuilder"
            timestamp="3213819318239"
            level="DEBUG"
            thread="Datafixer"
            >
            "#,
		)
		.unwrap();

		assert_eq!(
			attributes,
			Attributes {
				logger: "com.mojang.datafixers.DataFixerBuilder",
				level: LogLevel::Debug,
				timestamp: 3_213_819_318_239,
				thread_name: "Datafixer"
			}
		);
	}

	#[test]
	fn parse_logger_attribute() {
		let (_, attr) = attr_logger(r#"logger="com.mojang.datafixers.DataFixerBuilder""#).unwrap();

		assert_eq!(
			attr,
			Attribute::Logger("com.mojang.datafixers.DataFixerBuilder")
		);
	}

	#[test]
	fn parse_level_attribute() {
		let (_, attr) = attr_level(r#"level="INFO""#).unwrap();
		assert_eq!(attr, Attribute::Level(LogLevel::Info));
	}

	#[test]
	fn parse_timestamp_attribute() {
		let (_, attr) = attr_timestamp(r#"timestamp="313213473127""#).unwrap();
		assert_eq!(attr, Attribute::Timestamp(313_213_473_127));
	}

	#[test]
	fn parse_thread_attribute() {
		let (_, attr) = attr_thread(r#"thread="Datafixer""#).unwrap();
		assert_eq!(attr, Attribute::ThreadName("Datafixer"));
	}

	#[test]
	fn parse_quoted_string() {
		let (_, res) = quoted_string(r#""girlskissing""#).unwrap();

		assert_eq!(res, "girlskissing");
	}

	#[test]
	fn parse_single_entry() {
		let (_, entry) = parse_logentry(
			r#"
            <log4j:Event
                logger="com.mojang.datafixers.DataFixerBuilder"
                timestamp="3213712731731"
                level="DEBUG"
                thread="Datafixer"
            >
                <log4j:Message>
                    <![CDATA[200 Datafixer optimizations took 2000 years]]>
                </log4j:Message>
            </log4j:Event>
            "#,
		)
		.unwrap();

		assert_eq!(
			entry,
			LogEntry {
				logger: "com.mojang.datafixers.DataFixerBuilder",
				level: LogLevel::Debug,
				timestamp: 3_213_712_731_731,
				thread_name: "Datafixer",
				message: "200 Datafixer optimizations took 2000 years",
			}
		);
	}

	// #[test]
	// fn parse_sample_log_entries() {
	//     let mut input = include_str!("../../fixtures/test_log.xml");

	//     while let Ok((o, _)) = parse_logentry(input) {
	//         input = o;
	//     }

	//     assert_eq!(input, "exit code: 0");
	// }
}
