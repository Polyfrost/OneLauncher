import { A } from '@solidjs/router';
import { ChevronRightIcon, Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For, Show } from 'solid-js';
import { BrowserToolbar } from './BrowserRoot';
import type { ProviderSearchResults, Providers } from '~bindings';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import useBrowser from '~ui/hooks/useBrowser';
import { abbreviateNumber } from '~utils';

function BrowserMain() {
	const browser = useBrowser();

	return (
		<div class="relative h-full flex flex-1 flex-col gap-2">
			<BrowserToolbar />

			<ScrollableContainer>
				<div class="flex flex-col gap-4 py-2">
					<Show when={browser.cache() !== undefined}>
						<ModsRow header="Modrinth" category="modrinth" {...browser.cache()!} />
						<ModsRow header="Curseforge" category="curseforge" {...browser.cache()!} />
						<ModsRow header="Polyfrost" category="polyfrost" {...browser.cache()!} />
						<ModsRow header="Skyclient" category="skyclient" {...browser.cache()!} />
					</Show>
				</div>
			</ScrollableContainer>

		</div>
	);
}

type ModsRowProps = ProviderSearchResults & {
	header: string;
	category: string;
};

function ModsRow(props: ModsRowProps) {
	return (
		<div class="flex flex-1 flex-col gap-3">
			<div class="flex flex-1 flex-row justify-between">
				<h4>{props.header}</h4>
			</div>

			<OverlayScrollbarsComponent options={{ scrollbars: { autoHide: 'leave', autoHideDelay: 200 } }} class="max-w-full w-full overflow-hidden">
				<div class="flex flex-row gap-2">
					<For each={props.results}>
						{mod => (
							<ModCard
								title={mod.title}
								author={mod.author}
								description={mod.description}
								icon_url={mod.icon_url || ''}
								id={mod.project_id}
								provider={props.provider}
								downloads={mod.downloads}
								followers={mod.follows}
							/>
						)}
					</For>

					<div class="flex flex-col items-center justify-center px-6">
						<A class="whitespace-nowrap rounded-md px-4 py-2 text-lg text-fg-secondary hover:bg-gray-05 active:text-fg-secondary-pressed hover:text-fg-primary-hover" href={`category?category=${props.category}`}>
							<ChevronRightIcon class="" />
						</A>
					</div>
				</div>
			</OverlayScrollbarsComponent>

		</div>
	);
}

interface ModCardProps {
	title: string;
	author: string;
	description: string;
	icon_url: string;
	id: string;
	provider: Providers;
	downloads: number;
	followers: number;
};

function ModCard(props: ModCardProps) {
	const controller = useBrowser();

	function redirect() {
		controller.displayPackage(props.id, props.provider);
	}

	return (
		<div
			tabIndex={0}
			onClick={redirect}
			class="h-full max-h-72 max-w-53 min-h-72 min-w-53 w-full flex flex-col overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover"
		>
			<div class="relative h-28 flex items-center justify-center overflow-hidden">
				<img class="absolute z-0 max-w-none w-7/6 filter-blur-xl" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
				<img class="relative z-1 aspect-ratio-square w-2/5 rounded-md image-render-auto" src={props.icon_url || ''} alt={`Icon for ${props.title}`} />
			</div>
			<div class="flex flex-1 flex-col gap-2 p-3">
				<div class="flex flex-col gap-2">
					<h4 class="text-fg-primary font-medium">{props.title}</h4>
					<p class="text-xs text-fg-secondary">
						By
						{' '}
						<span class="text-fg-primary">{props.author}</span>
						{' '}
						on
						{' '}
						<span class="text-fg-primary">{props.provider}</span>
					</p>
				</div>

				<p class="max-h-22 flex-1 overflow-hidden text-sm text-fg-secondary line-height-snug">{props.description}</p>

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

export default BrowserMain;
