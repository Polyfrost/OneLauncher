import { createResource } from 'solid-js';
import type { JSX } from 'solid-js';
import steveSrc from '../../../assets/images/steve.png';

type PlayerHeadProps = JSX.IntrinsicElements['img'] & { uuid: string };

async function fetchHeadSrc(uuid: string) {
	return `https://crafatar.com/avatars/${uuid}?size=32`;
}

function PlayerHead(props: PlayerHeadProps) {
	const [headSrc] = createResource(() => props.uuid, fetchHeadSrc, {
		initialValue: steveSrc,
	});

	return <img src={headSrc()} {...props} />;
}

export default PlayerHead;
