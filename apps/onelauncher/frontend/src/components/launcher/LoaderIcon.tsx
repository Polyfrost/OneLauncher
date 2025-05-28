import type { Model, GameLoader } from '@/bindings.gen';
import FabricImage from '@/assets/logos/loaders/fabric.png';
import ForgeImage from '@/assets/logos/loaders/forge.png';
import NeoForgeImage from '@/assets/logos/loaders/neoforge.png';
import QuiltImage from '@/assets/logos/loaders/quilt.png';
import VanillaImage from '@/assets/logos/minecraft.png';
import type { RefAttributes } from 'react';

export function getLoaderLogoSrc(loader: Model | GameLoader): string {
	const loaderName = (typeof loader === 'string' ? loader : loader.mc_loader)?.toLowerCase() as GameLoader;

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

type LoaderIconProp = RefAttributes<HTMLImageElement> & {
	loader: GameLoader | undefined;
    className?: string;
};

function LoaderIcon(props: LoaderIconProp) {
	return (
		<img
			{...props}
			alt={`${props.loader}'s logo`}
			src={props.loader ? getLoaderLogoSrc(props.loader) : VanillaImage}
		/>
	);
}

export default LoaderIcon;
