import { A } from '@solidjs/router';
import { ChevronRightIcon } from '@untitled-theme/icons-solid';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For, Show } from 'solid-js';
import { BrowserToolbar } from './BrowserRoot';
import type { ProviderSearchResults } from '~bindings';
import ModCard from '~ui/components/content/ModCard';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import useBrowser from '~ui/hooks/useBrowser';

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

export default BrowserMain;
