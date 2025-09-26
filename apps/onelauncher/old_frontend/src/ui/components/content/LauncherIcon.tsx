import type { ImportType } from '@onelauncher/client/bindings';
import CurseForgeIcon from '~assets/logos/curseforge.svg?component-solid';
import AtLauncherIcon from '~assets/logos/launchers/atlauncher.svg?component-solid';
import FTBIcon from '~assets/logos/launchers/ftb.svg?component-solid';
import GDLauncherImage from '~assets/logos/launchers/gdlauncher.png';
import MultiMCImage from '~assets/logos/launchers/multimc.png';
import PrismIcon from '~assets/logos/launchers/prismlauncher.svg?component-solid';
import TechnicImage from '~assets/logos/launchers/technic.png';
import TLauncherImage from '~assets/logos/launchers/tlauncher.png';
import UnknownImage from '~assets/logos/minecraft.png';
import ModrinthIcon from '~assets/logos/modrinth.svg?component-solid';
import { type JSX, type JSXElement, Show, splitProps } from 'solid-js';

type ElementType = (props: any) => JSXElement;

export function getLauncherLogoElement(launcher: ImportType): ElementType {
	const providerName = launcher.toLowerCase() as Lowercase<ImportType>;

	const image = (src: string, props: any) => <img {...props} alt={providerName} src={src} />;

	// [provider logo, is svg]
	const mapping: Record<Lowercase<ImportType>, ElementType | string> = {
		modrinth: ModrinthIcon,
		curseforge: CurseForgeIcon,
		prismlauncher: PrismIcon,
		atlauncher: AtLauncherIcon,
		gdlauncher: GDLauncherImage,
		ftblauncher: FTBIcon,
		multimc: MultiMCImage,
		tlauncher: TLauncherImage,
		technic: TechnicImage,
		unknown: UnknownImage,
	};

	const logo = mapping[providerName] || UnknownImage;
	if (typeof logo === 'function')
		return logo;

	return props => image(logo, props);
}

type LauncherIconProps = JSX.HTMLAttributes<HTMLImageElement> & {
	launcher: ImportType | undefined;
};

function LauncherIcon(props: LauncherIconProps) {
	const [split, rest] = splitProps(props, ['launcher', 'class']);

	return (
		<Show
			children={getLauncherLogoElement(split.launcher!)({ ...rest, ...(split.class ? { class: split.class } : {}) })}
			fallback={(
				<div class={`bg-border/05 ${split.class || ''}`} {...rest as JSX.HTMLAttributes<HTMLDivElement>} />
			)}
			when={split.launcher !== undefined}
		/>
	);
}

export default LauncherIcon;
