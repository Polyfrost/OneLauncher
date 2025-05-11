import type { ImgHTMLAttributes } from 'react';
import SteveHead from '@/assets/images/steve.jpg';
import { useMemo } from 'react';

interface PlayerHeadProps {
	uuid?: string | undefined | null;
}

function PlayerHead({
	uuid,
	src: _src, // ignore src prop to prevent overriding the generated src
	...rest
}: PlayerHeadProps & ImgHTMLAttributes<HTMLImageElement>) {
	const src = useMemo(() => {
		return uuid
			? `https://crafatar.com/avatars/${uuid}?overlay&size=8`
			: SteveHead;
	}, [uuid]);

	return (
		<img
			{...rest}
			alt="Player Head"
			onError={(e) => {
				(e.target as HTMLImageElement).src = SteveHead;
			}}
			src={src}
			style={{ imageRendering: 'pixelated' }}
		/>
	);
}

export default PlayerHead;
