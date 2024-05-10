import { type ParentProps, createSignal, onCleanup, onMount } from 'solid-js';

type TimeAgoProps = ParentProps & {
	timestamp: number;
};

function TimeAgo(props: TimeAgoProps) {
	const [time, setTime] = createSignal('');
	const [intervalId, setIntervalId] = createSignal<NodeJS.Timeout | undefined>(undefined);

	onMount(() => {
		const formatter = new Intl.RelativeTimeFormat(navigator.language, { numeric: 'auto' });
		const units = {
			year: 24 * 60 * 60 * 1000 * 365,
			month: 24 * 60 * 60 * 1000 * 365 / 12,
			day: 24 * 60 * 60 * 1000,
			hour: 60 * 60 * 1000,
			minute: 60 * 1000,
			second: 1000,
		};

		const getRelativeTime = (timestamp: number): string => {
			const elapsed = timestamp - Date.now();

			for (const [unit, ms] of Object.entries(units)) {
				if (Math.abs(elapsed) > ms || unit === 'second')
					return formatter.format(Math.round(elapsed / ms), unit as Intl.RelativeTimeFormatUnit);
			}

			return 'now';
		};

		const loop = () => {
			setTime(getRelativeTime(props.timestamp));
		};

		loop();
		setIntervalId(setInterval(loop, 1000));
	});

	onCleanup(() => {
		clearInterval(intervalId()!);
	});

	return <span>{time()}</span>;
}

export default TimeAgo;
