import { Show, createSignal, splitProps } from 'solid-js';
import type { JSX } from 'solid-js';
import steveSrc from '../../../assets/images/steve.png';

type PlayerHeadProps = JSX.IntrinsicElements['img'] & {
	uuid: string;
	onError?: () => any;
};

function headSrc(uuid: string) {
	return `https://crafatar.com/avatars/${uuid}?size=48`;
}

function PlayerHead(props: PlayerHeadProps) {
	const [split, rest] = splitProps(props, ['uuid', 'class']);
	const [isLoading, setLoading] = createSignal(true);

	return (
		<>
			<Show when={isLoading() === true}>
				<img
					src={steveSrc}
					class={`image-render-pixel ${split.class}`}
					{...rest}
				/>
			</Show>
			<img
				src={headSrc(split.uuid)}
				onLoad={() => setLoading(false)}
				onError={() => props.onError && props.onError()}
				class={`image-render-pixel ${isLoading() ? 'hidden' : ''} ${split.class}`}
				{...rest}
			/>
		</>
	);
}

export default PlayerHead;
