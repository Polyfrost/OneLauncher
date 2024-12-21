import type { Cluster, Loader } from '@onelauncher/client/bindings';
import FabricImage from '~assets/logos/loaders/fabric.png';
import ForgeImage from '~assets/logos/loaders/forge.png';
import NeoForgeImage from '~assets/logos/loaders/neoforge.png';
import QuiltImage from '~assets/logos/loaders/quilt.png';
import VanillaImage from '~assets/logos/minecraft.png';
import { type JSX, splitProps } from 'solid-js';

export function getLoaderLogoSrc(loader: Cluster | Loader): string {
	const loaderName = (typeof loader === 'string' ? loader : loader.meta.loader)?.toLowerCase() as Loader;

	const mapping: Record<Loader, string> = {
		vanilla: VanillaImage,
		fabric: FabricImage,
		legacyfabric: FabricImage,
		forge: ForgeImage,
		neoforge: NeoForgeImage,
		quilt: QuiltImage,
	};

	return mapping[loaderName];
}

type LoaderIconProp = JSX.HTMLAttributes<HTMLImageElement> & {
	loader: Loader | undefined;
};

function LoaderIcon(props: LoaderIconProp) {
	const [{ loader = 'vanilla' }, rest] = splitProps(props, ['loader']);
	return (
		<img {...rest} alt={`${loader}'s logo`} src={getLoaderLogoSrc(loader)} />
	);
}

export default LoaderIcon;
