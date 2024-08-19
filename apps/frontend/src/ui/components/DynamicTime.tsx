import { type ParentProps, createSignal, onCleanup, onMount } from 'solid-js';
import { formatAsRelative } from '~utils';

type TimeProps = ParentProps & {
	timestamp: Date | number;
};

export function TimeAgo(props: TimeProps) {
	const getRelativeTime = (timestamp: number): string => {
		return formatAsRelative(timestamp);
	};

	return <InternalTime {...props} format={getRelativeTime} />;
}

type InternalTimeProps = TimeProps & {
	format: (time: number) => string;
};

function InternalTime(props: InternalTimeProps) {
	const [time, setTime] = createSignal('');
	const [intervalId, setIntervalId] = createSignal<NodeJS.Timeout | undefined>(undefined);

	onMount(() => {
		const timestamp = typeof props.timestamp === 'number' ? props.timestamp : props.timestamp.getTime();

		const loop = () => setTime(props.format(timestamp));
		loop();

		setIntervalId(setInterval(loop, 1000));
	});

	onCleanup(() => {
		clearInterval(intervalId()!);
	});

	return <span>{time()}</span>;
}
