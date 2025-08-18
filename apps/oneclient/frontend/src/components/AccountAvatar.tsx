import type { ImgHTMLAttributes } from 'react';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { getSkinUrl } from '@/utils/minecraft';
import { renderHeadToDataUrl } from '@onelauncher/common';
import { useEffect, useState } from 'react';

export interface AccountAvatarProps extends ImgHTMLAttributes<HTMLImageElement> {
	uuid: string | undefined | null;
}

export function AccountAvatar({
	uuid,
	alt,
	style,
	...rest
}: AccountAvatarProps) {
	const { data: profile } = usePlayerProfile(uuid);
	const [headUrl, setHeadUrl] = useState<string | null>(null);

	useEffect(() => {
		renderHeadToDataUrl(getSkinUrl(profile?.skin_url)).then(setHeadUrl);
	}, [profile]);

	return (
		<img
			alt={alt || `${profile?.username}'s avatars`}
			src={headUrl || undefined}
			style={{
				...style,
				imageRendering: 'pixelated',
			}}
			{...rest}
		/>
	);
}
