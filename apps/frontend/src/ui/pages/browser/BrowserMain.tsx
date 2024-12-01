import type { Providers } from '@onelauncher/client/bindings';
import { ChevronRightIcon } from '@untitled-theme/icons-solid';
import OneConfigLogo from '~assets/logos/oneconfig.svg?component-solid';
import Button from '~ui/components/base/Button';
import SearchResultsContainer from '~ui/components/content/SearchResults';
import useBrowser from '~ui/hooks/useBrowser';
import { For, onMount, Show } from 'solid-js';
import { BrowserContent } from './BrowserRoot';

function BrowserMain() {
	const browser = useBrowser();

	onMount(() => {
		browser.setSearchQuery(prev => ({
			...prev,
			categories: [],
		}));
	});

	return (
		<BrowserContent>
			<div class="flex flex-col gap-8">
				<Show when={browser.popularPackages() !== undefined && browser.featuredPackage() !== undefined}>
					<Featured package={browser.featuredPackage()! as unknown as FeaturedProps['package']} />

					<For each={Object.entries(browser.popularPackages()!)}>
						{([provider, results]) => (
							<SearchResultsContainer
								collapsable
								header={provider}
								provider={provider as Providers}
								results={results}
							/>
						)}
					</For>
				</Show>
			</div>
		</BrowserContent>
	);
}

interface FeaturedProps {
	package: {
		title: string;
		description: string;
		id: string;
		provider: Providers;
		oneconfig: boolean;
	};
}

function Featured(props: FeaturedProps) {
	const browser = useBrowser();

	const open = () => {
		browser.displayPackage(props.package.id, props.package.provider);
	};

	return (
		<div class="flex flex-col gap-y-1">
			<h5 class="ml-2">Featured</h5>
			<div class="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
				<div class="w-full p-1">
					<img alt="" class="aspect-ratio-video w-full rounded-md object-cover object-center" src="" />
				</div>
				<div class="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
					<h2>{props.package.title}</h2>

					<div class="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-border/10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
						<OneConfigLogo class="h-3.5 w-3.5" />
						<span class="text-sm font-medium">OneConfig Integrated</span>
					</div>

					<p class="mt-1 flex-1">Lorem ipsum, dolor sit amet consectetur adipisicing elit. Aliquid, veniam odio. Animi quia corporis id fugiat, libero commodi. Repudiandae repellat placeat sed tempora molestias id consequuntur, corrupti ullam mollitia amet?</p>

					<div class="flex flex-row justify-end">
						<Button
							buttonStyle="ghost"
							children="View Package"
							iconRight={<ChevronRightIcon />}
							onClick={open}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}

export default BrowserMain;
