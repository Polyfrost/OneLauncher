import { type JSX, splitProps } from 'solid-js';
import fabric from '~assets/logos/fabric.png';
import forge from '~assets/logos/forge.png';
import quilt from '~assets/logos/quilt.png';
import vanilla from '~assets/logos/vanilla.png';
import type { Loader } from '~bindings';

export function getLoaderIcon(loader: Loader): string {
	switch (loader) {
		case 'forge':
			return forge;
		case 'fabric':
			return fabric;
		case 'quilt':
			return quilt;
		case 'vanilla':
		default:
			return vanilla;
	}
}

type LoaderIconProp = JSX.HTMLAttributes<HTMLImageElement> & {
	loader: Loader | undefined;
};

function LoaderIcon(props: LoaderIconProp) {
	const [{ loader = 'vanilla' }, rest] = splitProps(props, ['loader']);
	return (
		<img {...rest} src={getLoaderIcon(loader)} alt={loader} />
	);
}

export default LoaderIcon;
