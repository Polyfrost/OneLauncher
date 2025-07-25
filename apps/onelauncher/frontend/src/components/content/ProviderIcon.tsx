import type { ImgHTMLAttributes, RefAttributes } from 'react';
import CurseForgeIcon from '@/assets/logos/curseforge.svg';
import VanillaImage from '@/assets/logos/minecraft.png';
import ModrinthIcon from '@/assets/logos/modrinth.svg';
import SkyClientImage from '@/assets/logos/skyclient.png';

export function getProviderLogoSrc(provider: string): string {
	const providerName = provider.toLowerCase();

	const mapping: Record<string, string> = {
		modrinth: ModrinthIcon,
		curseforge: CurseForgeIcon,
		skyclient: SkyClientImage,
	};

	return mapping[providerName];
}

type ProviderIconProp = ImgHTMLAttributes<HTMLImageElement> & RefAttributes<HTMLImageElement> & {
	provider: string | undefined;
};

function ProviderIcon({
	provider,
	src: _src,
	alt: _alt,
	...rest
}: ProviderIconProp) {
	return (
		<img
			{...rest}
			alt={`${provider}'s logo`}
			src={provider ? getProviderLogoSrc(provider) : VanillaImage}
		/>
	);
}

export default ProviderIcon;
