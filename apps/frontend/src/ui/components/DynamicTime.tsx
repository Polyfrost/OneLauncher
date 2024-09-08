import { formatAsRelative } from '~utils';
import { createSignal, onCleanup, onMount, type ParentProps } from 'solid-js';

type TimeProps = ParentProps & {
	timestamp: Date | number;
};

export const TimeAgo = (props: TimeProps) => <InternalTime {...props} format={formatAsRelative} />;

type InternalTimeProps = TimeProps & { format: (time: number) => string };

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
