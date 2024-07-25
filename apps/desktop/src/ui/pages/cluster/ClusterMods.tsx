import { ArrowRightIcon, ChevronDownIcon, ChevronUpIcon, Edit02Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { For, Match, Switch, createSignal, onMount } from 'solid-js';
import * as uuid from 'uuid';
import UFuzzy from '@leeoniya/ufuzzy';
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

const _mods: readonly ModEntryProps[] = Array(randomModNames.length).fill(undefined).map((_, index) => ({
	id: uuid.v4(),
	name: randomModNames[index] || 'Unknown',
	author: 'Author Name',
	version: '1.0.0',
	description: 'This is a mod description',
	provider: 'curseforge',
	thumbnail: 'https://cdn.modrinth.com/data/AANobbMI/icon.png',
}));

// TODO: Possibly optimise this as it has 2 cloned lists, and another containing only the names
function ClusterMods() {
	// Initial mods for this cluster
	const [mods] = createSignal<ModEntryProps[]>([..._mods]); // TODO: Replace with hook

	// Mods to display, should be a clone of the mods array but sorted
	const [displayedMods, setDisplayedMods] = createSignal<ModEntryProps[]>([]);

	// true - `A to Z` & false - `Z to A`
	const [sortingAtoZ, setSortingAtoZ] = createSignal<boolean>(true);

	const uf = new UFuzzy();
	const [modsSearchable] = createSignal(() => mods().map(mod => mod.name));

	function search(value: string) {
		if (value === '' || value === undefined) {
			resetMods();
			return;
		}

		const result = uf.search(modsSearchable()(), value);

		const filtered: ModEntryProps[] = [];
		result[0]?.forEach((index) => {
			const mod = mods()[index];
			if (mod)
				filtered.push(mod);
		});

		setDisplayedMods(sortListByName(filtered));
	}

	function sortListByName(list: ModEntryProps[]): ModEntryProps[] {
		if (sortingAtoZ())
			return list.sort((a, b) => a.name.localeCompare(b.name));
		else
			return list.sort((a, b) => b.name.localeCompare(a.name));
	}

	function toggleNameSort() {
		setSortingAtoZ(!sortingAtoZ());
		setDisplayedMods(sortListByName([...displayedMods()]));
	}

	function resetMods() {
		setDisplayedMods(sortListByName([...mods()]));
	}

	onMount(() => {
		resetMods();
	});

	return (
		<Sidebar.Page>
			<div class="flex flex-row justify-between w-full">
				<h1>Mods</h1>
				<div class="flex flex-row justify-end items-center gap-x-2">
					<Button
						buttonStyle="secondary"
						iconLeft={sortingAtoZ() ? <ChevronUpIcon /> : <ChevronDownIcon />}
						onClick={() => toggleNameSort()}
						children="Name"
					/>

					<TextField iconLeft={<SearchMdIcon />} placeholder="Search..." onInput={e => search(e.target.value)} />
				</div>
			</div>

			<ScrollableContainer>
				<Switch>
					<Match when={displayedMods().length > 0}>
						<For each={displayedMods()}>
							{mod => (
								<ModEntry {...mod} />
							)}
						</For>
					</Match>

					<Match when={displayedMods().length === 0}>
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
