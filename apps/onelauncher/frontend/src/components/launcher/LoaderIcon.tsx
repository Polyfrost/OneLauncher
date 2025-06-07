import type { GameLoader, Model } from '@/bindings.gen';
import type { ImgHTMLAttributes, RefAttributes } from 'react';
import FabricImage from '@/assets/logos/loaders/fabric.png';
import ForgeImage from '@/assets/logos/loaders/forge.png';
import NeoForgeImage from '@/assets/logos/loaders/neoforge.png';
import QuiltImage from '@/assets/logos/loaders/quilt.png';
import VanillaImage from '@/assets/logos/minecraft.png';

export function getLoaderLogoSrc(loader: Model | GameLoader): string {
	const loaderName = (typeof loader === 'string' ? loader : loader.mc_loader).toLowerCase() as GameLoader;

	const mapping: Record<GameLoader, string> = {
		vanilla: VanillaImage,
		fabric: FabricImage,
		legacyfabric: FabricImage,
		forge: ForgeImage,
		neoforge: NeoForgeImage,
		quilt: QuiltImage,
	};

	return mapping[loaderName];
}

type LoaderIconProp = ImgHTMLAttributes<HTMLImageElement> & RefAttributes<HTMLImageElement> & {
	loader: GameLoader | undefined;
};

function LoaderIcon({
	loader,
	src: _src,
	alt: _alt,
	...rest
}: LoaderIconProp) {
	return (
		<img
			{...rest}
			alt={`${loader}'s logo`}
			src={loader ? getLoaderLogoSrc(loader) : VanillaImage}
		/>
	);
}

export default LoaderIcon;
