import type { ManagedPackage, Providers } from '@onelauncher/client/bindings';
import CurseForgeIcon from '~assets/logos/curseforge.svg?component-solid';
import ModrinthImage from '~assets/logos/modrinth.svg?component-solid';
import SkyClientImage from '~assets/logos/skyclient.png';
import { type Component, type JSX, Match, Show, splitProps, Switch } from 'solid-js';

export function getProviderLogoElement(provider: ManagedPackage | Providers): string | Component {
	const providerName = (typeof provider === 'string' ? provider : provider.provider)?.toLowerCase() as Lowercase<Providers>;

	// [provider logo, is svg]
	const mapping: Record<Lowercase<Providers>, string | Component> = {
		modrinth: ModrinthImage,
		curseforge: CurseForgeIcon,
		skyclient: SkyClientImage,
	};

	return mapping[providerName];
}

type ProviderIconProps = JSX.HTMLAttributes<HTMLImageElement> & {
	provider: Providers | undefined;
};

function ProviderIcon(props: ProviderIconProps) {
	const [split, rest] = splitProps(props, ['provider', 'class']);

	const Element = () => {
		const value = getProviderLogoElement(split.provider!);

		return (
			<Switch>
				<Match when={typeof value === 'string'}>
					<img {...rest} {...(split.class ? { class: split.class } : {})} alt={`${split.provider}'s logo`} src={value as string} />
				</Match>
				<Match when>
					{value}
				</Match>
			</Switch>
		);
	};

	return (
		<Show
			children={(
				<Element />
			)}
			fallback={(
				<div class={`bg-border/05 ${split.class || ''}`} {...rest as JSX.HTMLAttributes<HTMLDivElement>} />
			)}
			when={split.provider !== undefined}
		/>
	);
}

export default ProviderIcon;
