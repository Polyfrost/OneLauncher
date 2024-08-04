import { A } from '@solidjs/router';
import { Download01Icon, FileCode01Icon, FilterLinesIcon, SearchMdIcon } from '@untitled-theme/icons-solid';
import { For, createSignal, onMount } from 'solid-js';
import type { ManagedPackage, Providers } from '~bindings';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import ModCard from '~ui/components/content/ModCard';
import { tryResult } from '~ui/hooks/useCommand';
import { createSortable } from '~utils';

interface CardProps {
	id: string;
	name: string;
	date_added: number;
	date_updated: number;
	provider: Providers;
};

interface BrowserFilters {
	provider: Providers[];
}

function BrowserMain() {
	const [_filters, _setFilters] = createSignal<BrowserFilters>({
		provider: ["Modrinth"],
	});

	const [modRowData, setModRowData] = createSignal<ModsRowProps[]>([]);

	onMount(() => {
		Promise.all([
			tryResult(bridge.commands.getPackage, 'oneconfig'),
			tryResult(bridge.commands.getPackage, 'chatting'),
			tryResult(bridge.commands.getPackage, 'patcher'),
			// tryResult(bridge.commands.randomMods),
		]).then((pkgs) => {
			const list: ModsRowProps[] = [
				{
					header: 'Polyfrost',
					category: 'polyfrost',
					packages: [
						pkgs[0],
						pkgs[1],
						pkgs[2],
					],
				},
				// {
				// 	header: 'Random Mods',
				// 	category: 'random',
				// 	packages: pkgs[3],
				// },
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
		<div class="flex flex-col flex-1 h-full gap-2 relative">
			<div class="flex flex-row justify-between sticky top-0 z-10 bg-primary">
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
		<div class="flex flex-col flex-1 gap-3">
			<div class="flex flex-row flex-1 justify-between">
				<h4>{props.header}</h4>
				<A class="text-fg-secondary hover:text-fg-secondary-hover active:text-fg-secondary-pressed" href={`category?category=${props.category}`}>See all</A>
			</div>

			<div class="flex flex-row gap-2 max-w-full flex-wrap">
				<For each={props.packages}>
					{mod => <ModCard {...mod} />}
				</For>
			</div>

		</div>
	);
}

export default BrowserMain;
