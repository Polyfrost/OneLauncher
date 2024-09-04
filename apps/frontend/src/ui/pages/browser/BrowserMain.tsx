import { ChevronRightIcon } from '@untitled-theme/icons-solid';
import { Show } from 'solid-js';
import type { ManagedPackage } from '@onelauncher/client/bindings';
import { BrowserContent } from './BrowserRoot';
import useBrowser from '~ui/hooks/useBrowser';
import OneConfigLogo from '~assets/logos/oneconfig.svg?component-solid';
import Button from '~ui/components/base/Button';
import SearchResultsContainer from '~ui/components/content/SearchResults';

function BrowserMain() {
	const browser = useBrowser();

	return (
		<BrowserContent>
			<div class="flex flex-col gap-8">
				<Show when={browser.cache() !== undefined && browser.featured() !== undefined}>
					<Featured package={browser.featured()!} />

					<SearchResultsContainer
						header="Modrinth"
						category="modrinth"
						collapsable
						{...browser.cache()!}
					/>
				</Show>
			</div>
		</BrowserContent>
	);
}

interface FeaturedProps {
	package: ManagedPackage;
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
							onClick={open}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}

export default BrowserMain;
