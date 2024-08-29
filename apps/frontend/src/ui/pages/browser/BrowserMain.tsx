import { ChevronRightIcon, Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import { For, Show } from 'solid-js';
import type { ManagedPackage, ProviderSearchResults, Providers } from '@onelauncher/client/bindings';
import { BrowserCategories, BrowserToolbar } from './BrowserRoot';
import useBrowser from '~ui/hooks/useBrowser';
import { abbreviateNumber } from '~utils';
import OneConfigLogo from '~assets/logos/oneconfig.svg?component-solid';
import Button from '~ui/components/base/Button';

function BrowserMain() {
	const browser = useBrowser();

	return (
		<div class="relative h-full flex flex-1 flex-col items-center gap-2">

			<div class="grid grid-cols-[160px_auto_160px] w-full max-w-screen-xl pb-8">
				<BrowserCategories />

				<div class="h-full flex flex-col gap-y-4">
					<BrowserToolbar />
					<div class="h-full flex-1">
						<div class="flex flex-col gap-8 py-2">
							<Show when={browser.cache() !== undefined && browser.featured() !== undefined}>
								<Featured package={browser.featured()!} />

								<ModsRow header="Modrinth" category="modrinth" {...browser.cache()!} />
							</Show>
						</div>
					</div>
				</div>
			</div>

		</div>
	);
}

interface FeaturedProps {
	package: ManagedPackage;
}

function Featured(props: FeaturedProps) {
	return (
		<div class="flex flex-col gap-y-1">
			<h5 class="ml-2">Featured</h5>
			<div class="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
				<div class="w-full p-1">
					<img class="aspect-ratio-video w-full rounded-md object-cover object-center" src="" alt="" />
				</div>
				<div class="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
					<h2>{props.package.title}</h2>

					<div class="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-gray-10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
						<OneConfigLogo class="h-3.5 w-3.5" />
						<span class="text-sm font-medium">OneConfig Integrated</span>
					</div>

					<p class="mt-1 flex-1">Lorem ipsum, dolor sit amet consectetur adipisicing elit. Aliquid, veniam odio. Animi quia corporis id fugiat, libero commodi. Repudiandae repellat placeat sed tempora molestias id consequuntur, corrupti ullam mollitia amet?</p>

					<div class="flex flex-row justify-end">
						<Button
							buttonStyle="ghost"
							iconRight={<ChevronRightIcon />}
							children="View Package"
							onClick={() => { }}
						/>
					</div>
				</div>
			</div>
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
				<h5 class="ml-2">{props.header}</h5>
			</div>

			<div class="grid grid-cols-3 gap-2 xl:grid-cols-4">
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
			</div>

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
			class="h-full max-h-72 min-h-72 min-w-53 flex flex-col overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover"
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
