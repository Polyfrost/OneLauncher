import { ArrowRightIcon, ChevronDownIcon, ChevronUpIcon, Edit02Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { For, Index, Match, Switch, createEffect, createSignal } from 'solid-js';
import * as uuid from 'uuid';
import uFuzzy from '@leeoniya/ufuzzy';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';

const randomModNames = [
	'Sodium',
	'Nvidium',
	'Iris Shaders',
	'Create',
	'Fabric API',
	'Create: Estrogen',
	'OneConfig',
	'PolySprint',
	'PolyEffects',
];

const mods: ModEntryProps[] = [];

for (let i = 0; i < randomModNames.length; i++) {
	mods.push({
		id: uuid.v4(),
		name: randomModNames[i] || 'Unknown',
		author: 'Author Name',
		version: '1.0.0',
		description: 'This is a mod description',
		provider: 'curseforge',
		thumbnail: 'https://cdn.modrinth.com/data/AANobbMI/icon.png',
	});
}

function ClusterMods() {
	// Search state:
	// `null` - No search query, show all mods
	// `[]` - Search query, but no results, show "no results" message
	// `[...]` - Search query, with results, show results
	const [indexes, setIndexes] = createSignal<number[] | null>(null);

	// State:
	// true - A to Z
	// false - Z to A
	const [sortingName, setSortingName] = createSignal<boolean>(true);

	const uf = new uFuzzy();
	const modsSearchable = mods
		.map(mod => mod.name);

	function search(value: string) {
		if (value === '' || value === undefined) {
			setIndexes(null);
			return;
		}

		const result = uf.search(modsSearchable, value);
		setIndexes(result[0] ?? []);
	}

	return (
		<Sidebar.Page>
			<div class="flex flex-row justify-between w-full">
				<h1>Mods</h1>
				<div class="flex flex-row justify-end items-center gap-x-2">
					<Button
						buttonStyle="secondary"
						iconLeft={sortingName() ? <ChevronUpIcon /> : <ChevronDownIcon />}
						onClick={() => setSortingName(!sortingName())}
						children="Name"
					/>

					<TextField iconLeft={<SearchMdIcon />} placeholder="Search..." onInput={e => search(e.target.value)} />
				</div>
			</div>

			<ScrollableContainer>
				<Switch>
					<Match when={indexes() === null}>
						<For each={mods}>
							{mod => (
								<ModEntry {...mod} />
							)}
						</For>
					</Match>
					<Match when={indexes()!.length > 0}>
						<For each={indexes()}>
							{(index) => {
								const mod = mods[index];
								return !mod ? <></> : <ModEntry {...mod} />;
							}}
						</For>
					</Match>
					<Match when={indexes()!.length === 0}>
						<p class="text-2lg text-center my-4">No mods were found</p>
					</Match>
				</Switch>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

export default ClusterMods;

// TODO: Mod
interface ModEntryProps {
	id: string;
	thumbnail: string;
	name: string;
	author: string;
	version: string;
	description: string;
	provider: 'curseforge' | 'modrinth' | 'polyfrost';
};

function ModEntry(props: ModEntryProps) {
	return (
		<div class="bg-component-bg hover:bg-component-bg-hover active:bg-component-bg-pressed p-3 gap-3 rounded-xl flex flex-row items-center">
			<div>
				<img src={props.thumbnail} alt={props.name} class="aspect-ratio-square h-10 rounded-lg" />
			</div>
			<div class="flex flex-col flex-1">
				<div class="flex flex-row justify-between items-center">
					<div class="flex flex-col items-start justify-center">
						<div class="flex flex-col items-start justify-center gap-y-2">
							<h4>{props.name}</h4>
							<span class="h-2 flex flex-row items-center justify-start text-xs font-600 text-fg-secondary/50">
								{props.version}
								{/* TODO: Add version checker */}
								{/* <Show when={props.version.includes()}> */}
								<ArrowRightIcon class="w-4 stroke-success" />
								<span class="text-success">1.0.1</span>
								{/* </Show> */}
							</span>
						</div>
					</div>

					<div class="flex flex-row gap-2 items-end justify-center">
						<Button buttonStyle="iconSecondary">
							<Edit02Icon />
						</Button>

						<Button buttonStyle="iconSecondary">
							<Trash03Icon class="!stroke-danger" />
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}
