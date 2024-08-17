import { Show, createEffect, createSignal, on, splitProps } from 'solid-js';
import type { JSX } from 'solid-js';
import steveSrc from '../../../assets/images/steve.png';

type PlayerHeadProps = JSX.IntrinsicElements['img'] & {
	uuid: string | null | undefined;
	onError?: () => any;
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
				when={props.uuid !== null && props.uuid !== undefined}
				fallback={(
					<img
						src={steveSrc}
						class={`image-render-pixel ${split.class}`}
						{...rest}
					/>
				)}
				children={(
					<img
						src={headSrc(split.uuid!)}
						onLoad={() => setLoaded(true)}
						onError={() => props.onError && props.onError()}
						class={`image-render-pixel ${isLoaded() ? '' : 'hidden'} ${split.class}`}
						{...rest}
					/>
				)}
			/>
		</>
	);
}

export default PlayerHead;
