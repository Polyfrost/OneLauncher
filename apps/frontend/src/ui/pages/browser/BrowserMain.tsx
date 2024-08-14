import { A } from '@solidjs/router';
import { Download01Icon, FileCode01Icon, FilterLinesIcon, SearchMdIcon } from '@untitled-theme/icons-solid';
import { For, createSignal, onMount } from 'solid-js';
import type { ManagedPackage } from '~bindings';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import ModCard, { Provider } from '~ui/components/content/ModCard';
import { tryResult } from '~ui/hooks/useCommand';
import { createSortable } from '~utils';

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
	const [_filters, _setFilters] = createSignal<BrowserFilters>({
		provider: [Provider.Curseforge],
	});

	const [modRowData, setModRowData] = createSignal<ModsRowProps[]>([]);

	onMount(() => {
		Promise.all([
			tryResult(bridge.commands.randomMods),
			tryResult(bridge.commands.getMod, 'oneconfig'),
			tryResult(bridge.commands.getMod, 'chatting'),
			tryResult(bridge.commands.getMod, 'patcher'),
		]).then((pkgs) => {
			const list: ModsRowProps[] = [
				{
					header: 'Polyfrost',
					category: 'polyfrost',
					packages: [
						pkgs[1],
						pkgs[2],
						pkgs[3],
					],
				},
				{
					header: 'Random Mods',
					category: 'random',
					packages: pkgs[0],
				},
			];

			setModRowData(list);
		});
	});

	const sortable = createSortable<CardProps>([], {
		'A-Z': (a, b) => a.name.localeCompare(b.name),
		'Z-A': (a, b) => b.name.localeCompare(a.name),
		'Last Updated': (a, b) => a.date_updated - b.date_updated,
		'New': (a, b) => b.date_added - a.date_added,
	});

	return (
		<div class="relative h-full flex flex-1 flex-col gap-2">
			<div class="sticky top-0 z-10 flex flex-row justify-between bg-primary">
				<div class="h-8 flex flex-row gap-2">

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
				<div class="flex flex-row justify-end gap-2">
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

			<div class="flex flex-col gap-4 py-2">
				<For each={modRowData()}>
					{row => (
						<ModsRow {...row} />
					)}
				</For>
			</div>

		</div>
	);
}

interface ModsRowProps {
	header: string;
	category: string;
	packages: ManagedPackage[];
};

function ModsRow(props: ModsRowProps) {
	return (
		<div class="flex flex-1 flex-col gap-3">
			<div class="flex flex-1 flex-row justify-between">
				<h4>{props.header}</h4>
				<A class="text-fg-secondary active:text-fg-secondary-pressed hover:text-fg-secondary-hover" href={`category?category=${props.category}`}>See all</A>
			</div>

			<div class="max-w-full flex flex-row flex-wrap gap-2">
				<For each={props.packages}>
					{mod => <ModCard {...mod} />}
				</For>
			</div>

		</div>
	);
}

export default BrowserMain;
