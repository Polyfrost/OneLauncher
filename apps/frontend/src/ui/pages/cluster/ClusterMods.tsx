import type { Package } from '@onelauncher/client/bindings';
import UFuzzy from '@leeoniya/ufuzzy';
import { open } from '@tauri-apps/plugin-shell';
import { FilterFunnel01Icon, LinkExternal01Icon, RefreshCw01Icon, SearchMdIcon, Trash03Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Checkbox from '~ui/components/base/Checkbox';
import TextField from '~ui/components/base/TextField';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useBrowser from '~ui/hooks/useBrowser';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import useProcessor from '~ui/hooks/useProcessor';
import useSettings from '~ui/hooks/useSettings';
import { join } from 'pathe';
import { type Accessor, createEffect, createSignal, For, Match, on, Show, Switch, untrack } from 'solid-js';

// TODO: This needs a rewrite.
function ClusterMods() {
	const [cluster] = useClusterContext();
	const { settings } = useSettings();
	const { isRunning } = useProcessor(cluster()!);
	const [mods, { refetch: refetchModsCommand }] = useCommand(cluster, () => bridge.commands.getClusterPackages(cluster()?.path || '', 'mod'));

	// Mods to display, should be a clone of the mods array but sorted
	const [displayedMods, setDisplayedMods] = createSignal<Package[]>([]);

	// true - `A to Z` & false - `Z to A`
	const [sortingAtoZ, setSortingAtoZ] = createSignal<boolean>(true);

	const [searchQuery, setSearchQuery] = createSignal<string>('');

	async function refetchMods() {
		if (mods.loading)
			return;

		await refetchModsCommand();
	}

	function getModName(mod: Package) {
		if (mod.meta.type === 'unknown')
			return mod.file_name;

		return mod.meta.title || mod.file_name;
	}

	const uf = new UFuzzy();
	const [modsSearchable] = createSignal(() => mods()?.map(getModName) || []);

	function search(query: string) {
		if (query.length === 0)
			return untrack(mods) || [];

		const result = uf.search(modsSearchable()(), query);

		const filtered: Package[] = [];
		result[0]?.forEach((index) => {
			const mod = mods()?.[index];
			if (mod)
				filtered.push(mod);
		});

		return filtered;
	}

	function sortListByName(list: Package[]): Package[] {
		if (sortingAtoZ())
			return list.sort((a, b) => getModName(a).localeCompare(getModName(b)));
		else
			return list.sort((a, b) => getModName(b).localeCompare(getModName(a)));
	}

	function onToggleNameSort() {
		setSortingAtoZ(value => !value);
		setDisplayedMods(value => sortListByName([...value]));
	}

	function openFolder() {
		open(join(settings().config_dir || '', 'clusters', cluster()?.path || '', 'mods'));
	}

	createEffect(on(mods, (value) => {
		setDisplayedMods(sortListByName(value || []));
	}));

	createEffect(on(searchQuery, (query) => {
		setDisplayedMods(sortListByName(search(query)));
	}));

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
							onClick={() => onToggleNameSort()}
						/>

						<TextField iconLeft={<SearchMdIcon />} onInput={e => setSearchQuery(e.target.value)} placeholder="Search..." />
					</div>
				</div>
			</div>

			<ScrollableContainer>
				<Switch>
					<Match when={mods.loading}>
						<p class="my-4 text-center text-2lg">Fetching mods...</p>
					</Match>

					<Match when={displayedMods().length > 0}>
						<For each={displayedMods()}>
							{mod => (
								<ModEntry isRunning={isRunning} pkg={mod} refetch={refetchMods} />
							)}
						</For>
					</Match>

					<Match when={displayedMods().length === 0}>
						<p class="my-4 text-center text-2lg">No mods were found</p>
					</Match>
				</Switch>
			</ScrollableContainer>

			<div class="mt-2 flex flex-row items-end justify-end">
				<Button
					buttonStyle="primary"
					children="Open Folder"
					iconLeft={<LinkExternal01Icon />}
					onClick={openFolder}
				/>
			</div>
		</Sidebar.Page>
	);
}

export default ClusterMods;

interface ModEntryProps {
	pkg: Package;
	refetch: () => void;
	isRunning: Accessor<boolean>;
};

function ModEntry(props: ModEntryProps) {
	const [cluster] = useClusterContext();
	const browser = useBrowser();
	// eslint-disable-next-line solid/reactivity -- .
	const [disabled, setDisabled] = createSignal(props.pkg.disabled || false);

	const name = () => {
		if (props.pkg.meta.type === 'unknown') {
			let fileName = props.pkg.file_name;
			if (fileName.endsWith('.disabled'))
				fileName = fileName.slice(0, -9);

			return fileName;
		}

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

	async function togglePackage(checked: boolean) {
		try {
			const path = cluster()?.path;
			if (path) {
				setDisabled(!checked);
				// eslint-disable-next-line solid/reactivity -- bs lol
				await tryResult(() => bridge.commands.setClusterPackageEnabled(
					path,
					props.pkg.file_name,
					'mod',
					checked,
				)).then(() => setDisabled(!checked));
			}
		}
		catch (err) {
			console.error(err);
		}
	}

	return (
		<div
			class="flex flex-row items-center gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover"
			classList={{
				'opacity-70 grayscale-50 hover:grayscale-0 hover:opacity-100': disabled(),
			}}
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

					<div class="flex flex-row items-center justify-center gap-2">
						<Checkbox defaultChecked={!props.pkg.disabled} onChecked={togglePackage} />

						<Button
							buttonStyle="iconDanger"
							children={<Trash03Icon />}
							disabled={props.isRunning()}
							onClick={deletePackage}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
