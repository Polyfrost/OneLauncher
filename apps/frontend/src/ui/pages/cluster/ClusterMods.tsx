import type { ManagedPackage, Package, Providers } from '@onelauncher/client/bindings';
import UFuzzy from '@leeoniya/ufuzzy';
import { FilterFunnel01Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useBrowser from '~ui/hooks/useBrowser';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import { createEffect, createResource, createSignal, For, Match, Show, Switch } from 'solid-js';

// TODO: Possibly optimise this as it has 2 cloned lists, and another containing only the names
function ClusterMods() {
	const [cluster] = useClusterContext();
	const [mods, { refetch }] = useCommand(cluster, () => bridge.commands.getClusterPackages(cluster()?.path || '', 'mod'));
	const [managedMods] = createResource(mods, async () => {
		const list = mods();
		if (!list)
			return Promise.resolve(new Map<Providers, ManagedPackage[]>());

		const providers: Map<Providers, string[]> = new Map();

		list.forEach((mod) => {
			if (mod.meta.type === 'managed')
				providers.set(mod.meta.provider, [
					...(providers.get(mod.meta.provider) || []),
					mod.meta.package_id,
				]);
		});

		const formatted: Map<Providers, ManagedPackage[]> = new Map();

		for (const [provider, packageIds] of providers.entries()) {
			const result = await tryResult(() => bridge.commands.getProviderPackages(provider, packageIds));
			formatted.set(provider, result);
		}

		return formatted;
	}, {
		initialValue: new Map<Providers, ManagedPackage[]>(),
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
				<div>
					<div class="flex flex-row items-stretch justify-end gap-x-2">
						<Button
							buttonStyle="secondary"
							children={sortingAtoZ() ? 'A-Z' : 'Z-A'}
							iconLeft={<FilterFunnel01Icon />}
							onClick={() => toggleNameSort()}
						/>

						<TextField iconLeft={<SearchMdIcon />} onInput={e => search(e.target.value)} placeholder="Search..." />
					</div>
				</div>
			</div>

			<ScrollableContainer>
				<Switch>
					<Match when={displayedMods().length > 0}>
						<For each={displayedMods()}>
							{mod => (
								<ModEntry managedMods={managedMods()} pkg={mod} refetch={refetch} />
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
	refetch: () => void;
	managedMods?: Map<Providers, ManagedPackage[]>;
};

function ModEntry(props: ModEntryProps) {
	const [cluster] = useClusterContext();
	const browser = useBrowser();

	const managed = () => {
		const pkg = props.pkg;
		if (props.managedMods && pkg.meta.type === 'managed') {
			const package_id = pkg.meta.package_id;
			return props.managedMods.get(pkg.meta.provider)?.find(p => p.id === package_id);
		}

		return undefined;
	};

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

	const icon = () => {
		if (props.pkg.meta.type === 'managed')
			return managed()?.icon_url;

		return undefined;
	};

	function onClick(e: MouseEvent) {
		const mngd = managed();

		if (mngd) {
			e.preventDefault();
			e.stopPropagation();

			browser.displayPackage(mngd.id, mngd.provider);
		}
	}

	async function deletePackage(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();

		const path = cluster()?.path;
		if (path) {
			await bridge.commands.removeClusterPackage(path, props.pkg.file_name, 'mod');
			await bridge.commands.syncClusterPackages(path);
			props.refetch();
		}
	}

	return (
		<div
			class="flex flex-row items-center gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover"
			onClick={onClick}
		>
			<div>
				<Show
					children={<img alt={name()} class="aspect-ratio-square h-10 rounded-lg" src={icon()!} />}
					fallback={<div class="aspect-ratio-square h-10 rounded-lg bg-gray-05" />}
					when={icon()}
				/>
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
						{/* <Button buttonStyle="iconSecondary">
							<Edit02Icon />
						</Button> */}

						<Button buttonStyle="iconDanger" onClick={deletePackage}>
							<Trash03Icon />
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}
