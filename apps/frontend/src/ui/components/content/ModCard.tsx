import { Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import { useNavigate } from '@solidjs/router';
import { abbreviateNumber } from '~utils';
import BrowserMod from '~ui/pages/browser/BrowserMod';
import type { ManagedPackage } from '~bindings';

// TODO: Remove in place for specta generated types
export enum Provider {
	Curseforge,
	Modrinth,
	Polyfrost,
	Skyclient,
}

function ModCard(props: ManagedPackage) {
	const navigate = useNavigate();

	function redirect() {
		navigate(BrowserMod.getUrl({
			id: props.id,
			provider: 'modrinth',
		}));
	}

	return (
		<div onClick={redirect} class="h-full max-h-68 max-w-53 min-h-68 min-w-53 w-full flex flex-col overflow-hidden rounded-lg bg-component-bg">
			<div class="relative h-28 flex items-center justify-center overflow-hidden">
				<img class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
				<img class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
			</div>
			<div class="flex flex-1 flex-col gap-4 p-3">
				<div class="flex flex-col gap-2">
					<h5 class="text-fg-primary font-medium">{props.title}</h5>
					<p class="text-xs text-fg-secondary">
						By
						{' '}
						<span class="text-fg-primary">Author TODO</span>
						{' '}
						on
						{' '}
						Modrinth
					</p>
				</div>

				<p class="max-h-13 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.description}</p>

				<div class="flex flex-row gap-4 text-xs">
					<div class="flex flex-row items-center gap-2">
						<Download01Icon class="h-4 w-4" />
						{abbreviateNumber(props.downloads)}
					</div>

					<div class="flex flex-row items-center gap-2">
						<HeartIcon class="h-4 w-4" />
						{abbreviateNumber(props.followers)}
					</div>
				</div>
			</div>
		</div>
	);
}

export default ModCard;
