import type { Providers, SearchResult } from '@onelauncher/client/bindings';
import type { JSX } from 'solid-js';
import { ChevronDownIcon, Download01Icon, HeartIcon } from '@untitled-theme/icons-solid';
import useBrowser from '~ui/hooks/useBrowser';
import useSettings from '~ui/hooks/useSettings';
import { abbreviateNumber } from '~utils';
import { createSignal, For, Match, Show, Switch } from 'solid-js';
import Button from '../base/Button';

interface SearchResultsContainerProps {
	provider: Providers;
	results: SearchResult[];
	header: string | JSX.Element;
	collapsable?: boolean;
}

function SearchResultsContainer(props: SearchResultsContainerProps) {
	const { settings } = useSettings();
	// eslint-disable-next-line solid/reactivity -- -
	const [collapsed, setCollapsed] = createSignal(props.collapsable || false);

	return (
		<div class="relative flex flex-1 flex-col gap-3">
			<div class="flex flex-1 flex-row justify-between">
				<Switch>
					<Match when={typeof props.header === 'string'}>
						<h5 class="ml-2">{props.header}</h5>
					</Match>

					<Match when>
						{props.header}
					</Match>
				</Switch>
			</div>

			<div
				class="w-full flex-1"
				classList={{
					'max-h-96 overflow-y-hidden relative after:(pointer-events-none content-[""] absolute z-1 top-0 left-0 w-full h-full bg-[linear-gradient(0deg,_theme(colors.page),transparent_25%)])': collapsed(),
				}}
			>
				<Switch>
					<Match when={props.results.length === 0}>
						<p class="mb-8 text-center text-fg-secondary">No results found</p>
					</Match>

					<Match when={settings().browser_list_view === 'list'}>
						<PackageList {...props} />
					</Match>

					<Match when>
						<PackageGrid {...props} />
					</Match>
				</Switch>
			</div>

			<Show when={collapsed() === true}>
				<div class="w-full flex items-center justify-center">
					<Button
						buttonStyle="secondary"
						children="Show more"
						iconLeft={<ChevronDownIcon />}
						onClick={() => setCollapsed(false)}
					/>
				</div>
			</Show>

		</div>
	);
}

export default SearchResultsContainer;

function PackageGrid(props: SearchResultsContainerProps) {
	return (
		<div class="grid grid-cols-2 grid-rows-1 gap-2 xl:grid-cols-3">
			<For each={props.results}>
				{mod => (
					<PackageItem {...mod} provider={props.provider} />
				)}
			</For>
		</div>
	);
}

function PackageList(props: SearchResultsContainerProps) {
	return (
		<div class="flex flex-col gap-2">
			<For each={props.results}>
				{mod => (
					<PackageItem {...mod} provider={props.provider} row />
				)}
			</For>
		</div>
	);
}

function PackageItem(props: SearchResult & { provider: Providers; row?: boolean }) {
	const controller = useBrowser();

	function redirect() {
		controller.displayPackage(props.project_id, props.provider);
	}

	return (
		<div
			class="h-full min-w-50 flex overflow-hidden rounded-lg bg-component-bg hover:bg-component-bg-hover"
			classList={{
				'flex-row max-h-32 min-h-32': props.row,
				'flex-col max-h-74 min-h-74': !props.row,
			}}
			onClick={redirect}
			tabIndex={0}
		>
			<div
				class="relative flex items-center justify-center overflow-hidden"
				classList={{
					'h-32 aspect-ratio-square': props.row,
					'w-full h-28': !props.row,
				}}
			>
				<Show
					fallback={(
						<div
							class="aspect-ratio-square rounded-md bg-gray-05"
							classList={{
								'w-2/5': !props.row,
								'w-3/4': props.row,
							}}
						/>
					)}
					when={props.icon_url}
				>
					<img alt={`Icon for ${props.title}`} class="absolute z-0 max-w-none w-7/6 opacity-50 filter-blur-xl" src={props.icon_url!} />
					<img
						alt={`Icon for ${props.title}`}
						class="relative z-1 aspect-ratio-square rounded-md image-render-auto"
						classList={{
							'w-2/5': !props.row,
							'w-3/4': props.row,
						}}
						src={props.icon_url!}
					/>
				</Show>
			</div>
			<div class="flex flex-1 flex-col gap-2 p-3">
				<div class="flex flex-col gap-2">
					<h4 class="text-fg-primary font-medium line-height-normal">{props.title}</h4>
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
						{abbreviateNumber(props.follows)}
					</div>
				</div>
			</div>
		</div>
	);
}
