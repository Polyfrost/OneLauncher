import type { Providers } from '@onelauncher/client/bindings';
import { ChevronRightIcon } from '@untitled-theme/icons-solid';
import OneConfigLogo from '~assets/logos/oneconfig.svg?component-solid';
import Button from '~ui/components/base/Button';
import SearchResultsContainer from '~ui/components/content/SearchResults';
import Spinner from '~ui/components/Spinner';
import useBrowser from '~ui/hooks/useBrowser';
import { createMemo, For, onMount, Show } from 'solid-js';
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
				<Featured />

				<Spinner.Suspense>
					<Show when={browser.popularPackages() !== undefined}>
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
				</Spinner.Suspense>
			</div>
		</BrowserContent>
	);
}

function Featured() {
	const browser = useBrowser();

	const featured = createMemo(() => browser.featuredPackage()?.[0]);

	const open = () => {
		const pkg = featured();
		if (!pkg)
			return;
		browser.displayPackage(pkg.id, pkg.provider);
	};

	return (
		<Show when={featured()}>
			<div class="flex flex-col gap-y-1">
				<h5 class="ml-2">Featured</h5>
				<div class="w-full flex flex-row overflow-hidden rounded-lg bg-page-elevated">
					<div class="w-full p-1">
						<img alt={`${featured()?.title} thumbnail`} class="aspect-ratio-video h-full w-full rounded-md object-cover object-center" src={featured()?.thumbnail} />
					</div>
					<div class="max-w-84 min-w-52 flex flex-col gap-y-1 p-4">
						<h2>{featured()?.title}</h2>

						<Show when={featured()?.oneconfig}>
							<div class="w-fit flex flex-row items-center gap-x-1 rounded-lg bg-border/10 px-1.5 py-1 text-fg-primary transition hover:opacity-80">
								<OneConfigLogo class="h-3.5 w-3.5" />
								<span class="text-sm font-medium">OneConfig Integrated</span>
							</div>
						</Show>

						<p class="mt-1 flex-1 leading-normal">{featured()?.description}</p>

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
		</Show>
	);
}

export default BrowserMain;
