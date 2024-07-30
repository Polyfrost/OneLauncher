import { For, createMemo } from 'solid-js';
import styles from './FormattedLog.module.scss';

interface FormattedLogProps {
	log: string;
};

function FormattedLog(props: FormattedLogProps) {
	const lines = createMemo(() => props.log.split('\n'));

	return (
		<code class={styles.log}>
			<For each={lines()}>
				{line => <Line line={line} />}
			</For>
		</code>
	);
}

interface LineProps {
	line: string;
};

/**
 * The first part of the pattern = `\[(\d+:\d+:\d+)\]`, attempts to find
 * and group `[DD:DD:DD]` where DD is a digit.
 * It matches only the formatted time (without the brackets)
 *
 * The second part of the pattern = `\[.[^\n\r/\u2028\u2029]*\/(.+)\]`,
 * attempts to find and group `[Thread name/LEVEL]`
 * where LEVEL is commonly either `INFO`, `WARN`, `ERROR`.
 * It only matches the Thread name and LEVEL
 *
 * The third part of the pattern = `: (?:\[(CHAT)\])?`,
 * attempts to find and group `: [CHAT]`.
 * It only matches `CHAT` if it is present.
 * This is an **optional** capture group.
 */
const REGEX_PATTERN = /\[(\d+:\d+:\d+)\] \[(.[^(\n\r/\u2028\u2029]*)\/(\w+)\]: (?:\[(CHAT)\])?/;

function Line(props: LineProps) {
	const format = (line: string) => {
		const prefix = line.match(REGEX_PATTERN);

		const isEmpty = line.trim() === '';

		if (isEmpty)
			line = '\n';

		if (prefix === null)
			return (
				<span
					{...(isEmpty ? { 'data-empty': 'true' } : {})}
				>
					{line}
				</span>
			);

		const isChatMsg = prefix[4] === 'CHAT';

		return (
			<span
				data-level={prefix[3]?.toUpperCase()}
				{...(isChatMsg ? { 'data-chat': 'true' } : {})}
				{...(isEmpty ? { 'data-empty': 'true' } : {})}
			>
				<span data-date={prefix[1]}>{`[${prefix[1]}]`}</span>
				<span>{` [${prefix[2]}/${prefix[3]}]: ${isChatMsg ? `[${prefix[4]}]` : ''}`}</span>
				<span>{line.slice(prefix[0].length)}</span>
			</span>
		);
	};

	return (
		<>{format(props.line)}</>
	);
}

export default FormattedLog;
