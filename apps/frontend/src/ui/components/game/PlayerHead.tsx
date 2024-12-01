import type { JSX } from 'solid-js';
import { createEffect, createSignal, on, Show, splitProps } from 'solid-js';
import steveSrc from '../../../assets/images/steve.png';

type PlayerHeadProps = JSX.IntrinsicElements['img'] & {
	uuid: string | null | undefined;
	onError?: () => void;
};

function headSrc(uuid: string) {
	return `https://crafatar.com/avatars/${uuid}?size=48`;
}

function PlayerHead(props: PlayerHeadProps) {
	const [split, rest] = splitProps(props, ['uuid', 'class']);
	const [isLoaded, setLoaded] = createSignal(false);

	createEffect(on(() => props.uuid, () => setLoaded(false)));

	return (
		<>
			<Show
				children={(
					<img
						class={`image-render-pixel ${isLoaded() ? '' : 'hidden'} ${split.class}`}
						onError={() => props.onError && props.onError()}
						onLoad={() => setLoaded(true)}
						src={headSrc(split.uuid!)}
						{...rest}
					/>
				)}
				fallback={(
					<img
						class={`image-render-pixel ${split.class}`}
						src={steveSrc}
						{...rest}
					/>
				)}
				when={props.uuid !== null && props.uuid !== undefined}
			/>
		</>
	);
}

export default PlayerHead;
