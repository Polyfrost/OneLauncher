import type { JSX } from 'solid-js';
import { createEffect, createSignal, splitProps } from 'solid-js';
import steveSrc from '../../../assets/images/steve.png';

type PlayerHeadProps = JSX.IntrinsicElements['img'] & {
	uuid: string | null | undefined;
	onError?: () => void;
};

function crafatar(uuid: string) {
	return `https://crafatar.com/avatars/${uuid}?size=48`;
}

function PlayerHead(props: PlayerHeadProps) {
	const [split, rest] = splitProps(props, ['uuid', 'class']);
	const [src, setSrc] = createSignal(steveSrc);

	createEffect(() => {
		if (split.uuid)
			setSrc(crafatar(split.uuid));
	});

	const onError = () => {
		setSrc(steveSrc);
		props.onError && props.onError();
	};

	return (
		<img
			class={`image-render-pixel ${split.class}`}
			onError={onError}
			src={src()}
			{...rest}
		/>
	);
}

export default PlayerHead;
