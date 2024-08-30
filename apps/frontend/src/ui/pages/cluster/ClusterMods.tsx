import { Edit02Icon, FilterFunnel01Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { For, Match, Switch, createEffect, createSignal } from 'solid-js';
import UFuzzy from '@leeoniya/ufuzzy';
import type { Package } from '@onelauncher/client/bindings';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand from '~ui/hooks/useCommand';
import { bridge } from '~imports';

// TODO: Possibly optimise this as it has 2 cloned lists, and another containing only the names
function ClusterMods() {
	const [cluster] = useClusterContext();
	const [mods] = useCommand(cluster, () => bridge.commands.getClusterPackages(cluster()?.path || '', 'mod'));

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
	const [modsSearchable] = createSignal(() => mods()?.map(getModName) || []);

	function search(value: string) {
		if (value === '' || value === undefined) {
			resetMods();
			return;
		}

		const result = uf.search(modsSearchable()(), value);

		const filtered: Package[] = [];
		result[0]?.forEach((index) => {
			const mod = mods()?.[index];
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
		setDisplayedMods(sortListByName([...(mods() || [])]));
	}

	createEffect(() => {
		resetMods();
	});

	return (
		<Sidebar.Page>
			<div class="w-full flex flex-row justify-between">
				<h1>Mods</h1>
				<div class="flex flex-row items-center justify-end gap-x-2">
					<Button
						buttonStyle="secondary"
						onClick={() => toggleNameSort()}
						children={sortingAtoZ() ? 'A-Z' : 'Z-A'}
						iconLeft={<FilterFunnel01Icon />}
					/>

					<TextField iconLeft={<SearchMdIcon />} placeholder="Search..." onInput={e => search(e.target.value)} />
				</div>
			</div>

			<ScrollableContainer>
				<Switch>
					<Match when={displayedMods().length > 0}>
						<For each={displayedMods()}>
							{mod => (
								<ModEntry pkg={mod} />
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

interface ModEntryProps {
	pkg: Package;
};

function icon() {
	return '';
}

function ModEntry(props: ModEntryProps) {
	const name = () => {
		if (props.pkg.meta.type === 'unknown')
			return props.pkg.file_name;

		return props.pkg.meta.title || props.pkg.file_name;
	};

	const version = () => {
		let formatted = 'Unknown';

		if (props.pkg.meta.type === 'managed')
			formatted = props.pkg.meta.version_formatted;
		else if (props.pkg.meta.type === 'mapped')
			formatted = props.pkg.meta.version || formatted;

		return formatted;
	};

	const tag = () => {
		let tag = 'Unknown';

		if (props.pkg.meta.type === 'managed')
			tag = props.pkg.meta.provider;
		else if (props.pkg.meta.type === 'mapped')
			tag = 'Mapped';

		return tag;
	};

	return (
		<div class="flex flex-row items-center gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover">
			<div>
				<img src={icon()} alt={name()} class="aspect-ratio-square h-10 rounded-lg" />
			</div>
			<div class="flex flex-1 flex-col">
				<div class="flex flex-row items-center justify-between">
					<div class="flex flex-col items-start justify-center">
						<div class="flex flex-col items-start justify-center gap-y-2">
							<span class="flex flex-row items-center justify-start gap-x-1">
								<h4>{name()}</h4>
								<span class="rounded-xl bg-gray-05 px-1.5 py-0.5 text-xs text-white/60">{tag()}</span>
							</span>
							<span class="h-2 flex flex-row items-center justify-start text-xs text-fg-secondary/50 font-600">
								{version()}
								{/* TODO: Add version checker */}
								{/* <Show when={props.version.includes()}> */}
								{/* <ArrowRightIcon class="w-4 stroke-success" />
								<span class="text-success">1.0.1</span> */}
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
