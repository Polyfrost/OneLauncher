import { Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import { useNavigate } from '@solidjs/router';
import { abbreviateNumber } from '../../../utils/primitives';
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
		<div onClick={redirect} class="flex flex-col overflow-hidden rounded-lg bg-component-bg max-w-53 min-w-53 w-full max-h-68 min-h-68 h-full">
			<div class="relative h-28 overflow-hidden flex justify-center items-center">
				<img class="absolute filter-blur-xl z-0 max-w-none w-7/6" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
				<img class="relative w-2/5 aspect-ratio-square z-1 rounded-md image-render-auto" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
			</div>
			<div class="flex flex-col flex-1 p-3 gap-4">
				<div class="flex flex-col gap-2">
					<h5 class="font-medium text-fg-primary">{props.title}</h5>
					<p class="text-fg-secondary text-xs">
						By
						{' '}
						<span class="text-fg-primary">Author TODO</span>
						{' '}
						on
						{' '}
						Modrinth
					</p>
				</div>

				<p class="text-fg-secondary text-sm flex-1 max-h-13 overflow-hidden line-height-snug">{props.description}</p>

				<div class="flex flex-row gap-4 text-xs">
					<div class="flex flex-row items-center gap-2">
						<Download01Icon class="w-4 h-4" />
						{abbreviateNumber(props.downloads)}
					</div>

					<div class="flex flex-row items-center gap-2">
						<HeartIcon class="w-4 h-4" />
						{abbreviateNumber(props.followers)}
					</div>
				</div>
			</div>
		</div>
	);
}

export default ModCard;
