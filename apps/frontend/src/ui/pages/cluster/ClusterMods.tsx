import type { Package } from '@onelauncher/client/bindings';
import UFuzzy from '@leeoniya/ufuzzy';
import { FilterFunnel01Icon, RefreshCw01Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useBrowser from '~ui/hooks/useBrowser';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand from '~ui/hooks/useCommand';
import { createEffect, createSignal, For, Match, Show, Switch } from 'solid-js';

// TODO: This needs a rewrite.
function ClusterMods() {
	const [cluster] = useClusterContext();
	const [mods, { refetch: refetchMods }] = useCommand(cluster, () => bridge.commands.getClusterPackages(cluster()?.path || '', 'mod'));

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
					<div class="h-8 flex flex-row items-stretch justify-end gap-x-2">
						<Button
							buttonStyle="iconSecondary"
							children={<RefreshCw01Icon />}
							onClick={() => {
								bridge.commands.syncClusterPackagesByType(cluster()?.path || '', 'mod', true).then(refetchMods);
							}}
						/>

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
								<ModEntry pkg={mod} refetch={refetchMods} />
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
};

function ModEntry(props: ModEntryProps) {
	const [cluster] = useClusterContext();
	const browser = useBrowser();

	const name = () => {
		if (props.pkg.meta.type === 'unknown')
			return props.pkg.file_name;

		return props.pkg.meta.title || props.pkg.file_name;
	};

	const version = () => {
		let formatted = 'Unknown';

		if (props.pkg.meta.type === 'managed')
			formatted = props.pkg.meta.version_formatted;

		return formatted;
	};

	const tags = () => {
		type TagStatus = 'normal' | 'warning' | 'danger';
		const tags: [string, TagStatus][] = [];
		const meta = props.pkg.meta;

		if (meta.type === 'managed') {
			tags[0] = [meta.provider, 'normal'];

			const supportedVersions = meta.mc_versions || [];
			if (supportedVersions.length === 0)
				tags.push(['Unknown Version', 'warning']);
			else if (!supportedVersions.includes(cluster()!.meta.mc_version))
				tags.push(['Incompatible', 'danger']);
		}
		else {
			tags[0] = ['Unknown', 'normal'];
		}

		return tags;
	};

	const icon = () => {
		if (props.pkg.meta.type === 'managed')
			return props.pkg.meta.icon_url;

		return undefined;
	};

	function onClick(e: MouseEvent) {
		const meta = props.pkg.meta;
		if (meta.type === 'managed') {
			e.preventDefault();
			e.stopPropagation();

			browser.displayPackage(meta.package_id, meta.provider);
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
					fallback={<div class="aspect-ratio-square h-10 rounded-lg bg-border/05" />}
					when={icon()}
				/>
			</div>
			<div class="flex flex-1 flex-col">
				<div class="flex flex-row items-center justify-between">
					<div class="flex flex-col items-start justify-center">
						<div class="flex flex-col items-start justify-center gap-y-2">
							<span class="flex flex-row items-center justify-start gap-x-1">
								<h4>{name()}</h4>
								<For each={tags()}>
									{([tag, status]) => (
										<span
											class="rounded-xl px-1.5 py-1 text-xs"
											classList={{
												'bg-border/05 text-white/60': status === 'normal',
												'bg-amber/10 text-amber': status === 'warning',
												'bg-danger/15 text-danger': status === 'danger',
											}}
										>
											{tag}
										</span>
									)}
								</For>
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
