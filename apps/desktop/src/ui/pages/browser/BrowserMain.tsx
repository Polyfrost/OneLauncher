import { Download01Icon, FileCode01Icon, FilterLinesIcon, SearchMdIcon } from '@untitled-theme/icons-solid';
import { For, createSignal } from 'solid-js';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import ModCard, { Provider } from '~ui/components/content/ModCard';
import createSortable from '~utils/sorting';

interface CardProps {
	id: string;
	name: string;
	date_added: number;
	date_updated: number;
	provider: Provider;
};

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

export const modStore: Mod[] = [
	{
		id: '1yIQcc2b',
		name: 'EvergreenHUD',
		description: 'Improves your heads up display.',
		author: 'Polyfrost',
		icon_url: 'https://cdn.modrinth.com/data/1yIQcc2b/icon.png',
		page_url: 'https://modrinth.com/mod/evergreenhud',
		provider: Provider.Modrinth,
		downloads: 281700,
		ratings: 220,
	},
];

const _modRowData: ModsRowProps[] = [
	{
		header: 'Highly Endorsed',
		category: 'endorsed',
		mods: modStore,
	},
];

interface ModsRowProps {
	header: string;
	category: string;
	mods: Mod[];
};

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

export default BrowserMain;
