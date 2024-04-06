import { type JSX, splitProps } from 'solid-js';
import fabric from '../../../assets/logos/fabric.png';
import forge from '../../../assets/logos/forge.png';
import quilt from '../../../assets/logos/quilt.png';
import vanilla from '../../../assets/logos/vanilla.png';

export function getLoaderIcon(loader: Core.Loader) {
	switch (loader) {
		case 'Vanilla':
			return vanilla;
		case 'Forge':
			return forge;
		case 'Fabric':
			return fabric;
		case 'Quilt':
			return quilt;
	}
}

type LoaderIconProp = JSX.HTMLAttributes<HTMLImageElement> & {
	loader: Core.Loader;
};

function LoaderIcon(props: LoaderIconProp) {
	const [split, rest] = splitProps(props, ['loader']);
	return (
		<img {...rest} src={getLoaderIcon(split.loader)} alt={split.loader} />
	);
}

export default LoaderIcon;
