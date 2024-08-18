import { ArrowRightIcon, ChevronDownIcon, ChevronUpIcon, Edit02Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { For, Match, Switch, createMemo, createSignal, onMount } from 'solid-js';
import UFuzzy from '@leeoniya/ufuzzy';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import type { Package } from '~bindings';

// TODO: Possibly optimise this as it has 2 cloned lists, and another containing only the names
function ClusterMods() {
	const [cluster] = useClusterContext();

	// Initial mods for this cluster
	const mods = createMemo(() => {
		return Object.values(cluster()?.packages || {})
			.filter(pkg => (pkg.meta.type === 'managed' || pkg.meta.type === 'mapped') && pkg.meta.package_type === 'mod');
	});

	// Mods to display, should be a clone of the mods array but sorted
	const [displayedMods, setDisplayedMods] = createSignal<Package[]>([]);

	// true - `A to Z` & false - `Z to A`
	const [sortingAtoZ, setSortingAtoZ] = createSignal<boolean>(true);

	function getModName(mod: Package) {
		if (mod.meta.type === 'unknown')
			return mod.file_name;

		return mod.meta.title || mod.file_name;
	}

	const uf = new UFuzzy();
	const [modsSearchable] = createSignal(() => mods().map(getModName) || []);

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

	function sortListByName(list: Package[]): Package[] {
		if (sortingAtoZ())
			return list.sort((a, b) => getModName(a).localeCompare(getModName(b)));
		else
			return list.sort((a, b) => getModName(b).localeCompare(getModName(a)));
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
			<div class="w-full flex flex-row justify-between">
				<h1>Mods</h1>
				<div class="flex flex-row items-center justify-end gap-x-2">
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
						<p class="my-4 text-center text-2lg">No mods were found</p>
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
		<div class="flex flex-row items-center gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover">
			<div>
				<img src={props.thumbnail} alt={props.name} class="aspect-ratio-square h-10 rounded-lg" />
			</div>
			<div class="flex flex-1 flex-col">
				<div class="flex flex-row items-center justify-between">
					<div class="flex flex-col items-start justify-center">
						<div class="flex flex-col items-start justify-center gap-y-2">
							<h4>{props.name}</h4>
							<span class="h-2 flex flex-row items-center justify-start text-xs text-fg-secondary/50 font-600">
								{props.version}
								{/* TODO: Add version checker */}
								{/* <Show when={props.version.includes()}> */}
								<ArrowRightIcon class="w-4 stroke-success" />
								<span class="text-success">1.0.1</span>
								{/* </Show> */}
							</span>
						</div>
					</div>

					<div class="flex flex-row items-end justify-center gap-2">
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
