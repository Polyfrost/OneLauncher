import type { ImgHTMLAttributes, JSX } from 'react';
import { Show } from '@onelauncher/common/components';
import CurseforgeIcon from '../../assets/logos/curseforge.svg';
import AtLauncherIcon from '../../assets/logos/launchers/atlauncher.svg';
import FTBIcon from '../../assets/logos/launchers/ftb.svg';
import GDLauncherImage from '../../assets/logos/launchers/gdlauncher.png';
import MultiMCImage from '../../assets/logos/launchers/multimc.png';
import PrismIcon from '../../assets/logos/launchers/prismlauncher.svg';
import TechnicImage from '../../assets/logos/launchers/technic.png';
import TLauncherImage from '../../assets/logos/launchers/tlauncher.png';
import UnknownImage from '../../assets/logos/minecraft.png';
import ModrinthIcon from '../../assets/logos/modrinth.svg';

type ElementType = (props: any) => JSX.Element;

// i dont know what ImportType is so i assume it is a string
export function getLauncherLogoElement(launcher: string): ElementType {
	const providerName = launcher.toLowerCase() as Lowercase<string>;

	const image = (src: string, props: any) => <img {...props} alt={providerName} src={src} />;

	// [provider logo, is svg]
	const mapping: Record<Lowercase<string>, ElementType | string> = {
		modrinth: ModrinthIcon,
		curseforge: CurseforgeIcon,
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

type LauncherIconProps = ImgHTMLAttributes<HTMLImageElement> & {
	launcher: string | undefined;
};

function LauncherIcon({
	launcher,
	...props
}: LauncherIconProps) {
	return (
		<Show
			children={getLauncherLogoElement(launcher!)({ ...props, ...(props.className ? { className: props.className } : {}) })}
			fallback={(
				<div className={`bg-border/05 ${props.className || ''}`} {...props} />
			)}
			when={launcher !== undefined}
		/>
	);
}

export default LauncherIcon;
