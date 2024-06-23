import { Download01Icon, FileCode01Icon, FilterLinesIcon, HeartIcon, SearchMdIcon } from '@untitled-theme/icons-solid';
import { type Accessor, For, type Setter, createEffect, createSignal } from 'solid-js';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import { abbreviateNumber } from '~utils/primitives';
import createSortable from '~utils/sorting';

interface CardProps {
	id: string;
	name: string;
	date_added: number;
	date_updated: number;
	provider: Provider;
};

enum Provider {
	Curseforge,
	Modrinth,
	Polyfrost,
	Skyclient,
}

interface BrowserFilters {
	provider: Provider[]; // TODO: Change with Provider types from Rust
}

function BrowserMain() {
	const [filters, setFilters] = createSignal<BrowserFilters>({
		provider: [Provider.Curseforge],
	});

	const sortable = createSortable<CardProps>([], {
		'A-Z': (a, b) => a.name.localeCompare(b.name),
		'Z-A': (a, b) => b.name.localeCompare(a.name),
		'Last Updated': (a, b) => a.date_updated - b.date_updated,
		'New': (a, b) => b.date_added - a.date_added,
	});

	return (
		<div class="flex flex-col flex-1 h-full gap-2 relative">
			<div class="flex flex-row flex-1 justify-between sticky top-0 z-10 bg-primary">
				<div class="flex flex-row gap-2 h-8">

					<TextField
						iconLeft={<SearchMdIcon />}
						placeholder="Search for content"
					/>

					<Dropdown.Minimal
						onChange={sortable.setKey}
						icon={<FilterLinesIcon />}
					>
						<For each={Object.keys(sortable.sortables)}>
							{sortable => (
								<Dropdown.Row>{sortable}</Dropdown.Row>
							)}
						</For>
					</Dropdown.Minimal>

				</div>
				<div class="flex flex-row gap-2 justify-end">
					<Button
						iconLeft={<Download01Icon />}
						children="From URL"
					/>
					<Button
						iconLeft={<FileCode01Icon />}
						children="From File"
						buttonStyle="secondary"
					/>
				</div>
			</div>

			<div class="flex flex-col h-full flex-1 gap-4 py-2">
				{/* eslint-disable-next-line ts/no-use-before-define -- blh */}
				<For each={_modRowData}>
					{row => (
						<>
							<ModsRow {...row} />
							<ModsRow {...row} />
							<ModsRow {...row} />
						</>
					)}
				</For>
			</div>

		</div>
	);
}

const _modRowData: ModsRowProps[] = [
	{
		header: 'Highly Endorsed',
		category: 'endorsed',
		mods: [
			{
				name: 'EvergreenHUD',
				description: 'Improves your heads up display.',
				author: 'Polyfrost',
				icon_url: 'https://cdn.modrinth.com/data/1yIQcc2b/icon.png',
				page_url: 'https://modrinth.com/mod/evergreenhud',
				provider: Provider.Modrinth,
				downloads: 281700,
				ratings: 220,
			},
		],
	},
];

interface ModsRowProps {
	header: string;
	category: string;
	mods: Mod[];
};

interface Mod {
	name: string;
	description: string;
	author: string;
	icon_url: string;
	provider: Provider;
	page_url: string;
	downloads: number;
	ratings: number;
}

function ModsRow(props: ModsRowProps) {
	return (
		<div class="flex flex-col flex-1 gap-3">
			<div class="flex flex-row flex-1 justify-between">
				<h4>{props.header}</h4>
				<a class="text-fg-secondary hover:text-fg-secondary-hover active:text-fg-secondary-pressed" href={`/browser/category?category=${props.category}`}>See all</a>
			</div>

			<div class="flex flex-row gap-2">
				<For each={props.mods}>
					{mod => <ModCard {...mod} />}
				</For>
			</div>

		</div>
	);
}

function ModCard(props: Mod) {
	return (
		<div class="flex flex-col overflow-hidden rounded-lg bg-component-bg max-w-53 min-w-53 w-full max-h-68 min-h-68 h-full">
			<div class="relative h-28 overflow-hidden flex justify-center items-center">
				<img class="absolute filter-blur-xl z-0 max-w-none w-7/6" src={props.icon_url} alt={`Icon for ${props.name}`} />
				<img class="relative w-2/5 aspect-ratio-square z-1 rounded-2xl" src={props.icon_url} alt={`Icon for ${props.name}`} />
			</div>
			<div class="flex flex-col flex-1 p-3 gap-4">
				<div class="flex flex-col gap-2">
					<h5 class="font-medium text-fg-primary">{props.name}</h5>
					<p class="text-fg-secondary text-xs">
						By
						{' '}
						<span class="text-fg-primary">{props.author}</span>
						{' '}
						on
						{' '}
						{Provider[props.provider]}
					</p>
				</div>

				<p class="text-fg-secondary text-sm flex-1">{props.description}</p>

				<div class="flex flex-row gap-4 text-xs">
					<div class="flex flex-row items-center gap-2">
						<Download01Icon class="w-4 h-4" />
						{abbreviateNumber(props.downloads)}
					</div>

					<div class="flex flex-row items-center gap-2">
						<HeartIcon class="w-4 h-4" />
						{abbreviateNumber(props.ratings)}
					</div>
				</div>
			</div>
		</div>
	);
}

export default BrowserMain;
