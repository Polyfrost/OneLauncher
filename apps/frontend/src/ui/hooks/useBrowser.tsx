import type { Cluster, ManagedPackage, PackageType, Providers, ProviderSearchQuery, ProviderSearchResults } from '@onelauncher/client/bindings';
import type { ModalProps } from '~ui/components/overlay/Modal';
import { useNavigate } from '@solidjs/router';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import BrowserPackage from '~ui/pages/browser/BrowserPackage';
import { type Accessor, type Context, createContext, createEffect, createSignal, For, on, onMount, type ParentProps, type Setter, Show, untrack, useContext } from 'solid-js';
import { useRecentCluster } from './useCluster';
import useCommand, { tryResult } from './useCommand';

export type ProviderSearchOptions = ProviderSearchQuery & { provider: Providers };

interface BrowserControllerType {
	cluster: Accessor<Cluster | undefined>;
	setCluster: Setter<Cluster | undefined>;

	displayBrowser: (cluster?: Cluster | undefined) => void;
	displayPackage: (id: string, provider: Providers) => void;
	displayCategory: (category: string) => void;
	displayClusterSelector: () => void;

	search: () => void;
	searchQuery: Accessor<ProviderSearchOptions>;
	setSearchQuery: Setter<ProviderSearchOptions>;

	packageType: Accessor<PackageType>;
	setPackageType: Setter<PackageType>;

	refreshCache: () => void;
	cache: Accessor<ProviderSearchResults | undefined>;
	featured: Accessor<ManagedPackage | undefined>;
};

const BrowserContext = createContext() as Context<BrowserControllerType>;

export function BrowserProvider(props: ParentProps) {
	const [mainPageCache, setMainPageCache] = createSignal<ProviderSearchResults>();
	const [featured, setFeatured] = createSignal<ManagedPackage | undefined>();

	// Used for the current "Browser Mode". It'll only show packages of the selected type
	const [packageType, setPackageType] = createSignal<PackageType>('mod');

	// Active cluster that results should be "focused" on
	const [cluster, setCluster] = createSignal<Cluster>();

	const [searchOptions, setSearchOptions] = createSignal<ProviderSearchOptions>({
		provider: 'Modrinth',
		query: '',
		limit: 20,
		offset: 0,
		categories: null,
		game_versions: null,
		loaders: null,
		package_types: ['mod'],
		open_source: null,
	});

	const navigate = useNavigate();
	const recentCluster = useRecentCluster();

	const modal = createModal(props => (
		<ChooseClusterModal
			{...props}
		/>
	));

	const controller: BrowserControllerType = {
		cluster,
		setCluster,

		displayBrowser(cluster?: Cluster) {
			setCluster(cluster);
		},

		displayPackage(id: string, provider: Providers) {
			navigate(BrowserPackage.buildUrl({ id, provider }));
		},

		displayCategory(_category: string) {

		},

		displayClusterSelector() {
			modal.show();
		},

		search: () => {
			navigate('/browser/search');
		},
		searchQuery: searchOptions,
		setSearchQuery: setSearchOptions,

		packageType,
		setPackageType,

		async refreshCache() {
			const opts = untrack(searchOptions);
			const res = await tryResult(() => bridge.commands.searchProviderPackages('Modrinth', opts));

			setMainPageCache(res);

			// TODO: Better algorithm for selecting a featured package
			const firstPackage = res.results[0];
			if (firstPackage !== undefined) {
				const featuredPackage = await tryResult(() => bridge.commands.getProviderPackage('Modrinth', firstPackage.project_id));
				setFeatured(featuredPackage);
			}
		},

		cache: mainPageCache,

		featured,
	};

	onMount(() => {
		if (mainPageCache() === undefined)
			controller.refreshCache();
	});

	createEffect(() => {
		setCluster(recentCluster());
	});

	createEffect(on(cluster, (cluster) => {
		if (cluster !== undefined)
			setSearchOptions((prev) => {
				const opts: ProviderSearchOptions = {
					...prev,
					game_versions: [cluster.meta.mc_version],
				};

				if (cluster.meta.loader !== undefined)
					opts.loaders = [cluster.meta.loader];

				return opts;
			});
	}));

	return (
		<BrowserContext.Provider value={controller}>
			{props.children}
		</BrowserContext.Provider>
	);
}

export default function useBrowser() {
	const controller = useContext(BrowserContext);

	if (!controller)
		throw new Error('useBrowserController should be called inside its BrowserProvider');

	return controller;
}

function ChooseClusterModal(props: ModalProps) {
	const [selected, setSelected] = createSignal<number>(0);
	const [clusters] = useCommand(() => bridge.commands.getClusters());
	const controller = useBrowser();

	const currentlySelectedCluster = (list: Cluster[]) => {
		const cluster = controller.cluster();

		if (cluster === undefined)
			return 0;

		if (list === undefined)
			return 0;

		const index = list.findIndex(c => c.uuid === cluster.uuid) || 0;
		if (index === -1)
			return 0;

		return index;
	};

	createEffect(() => {
		setSelected(currentlySelectedCluster(clusters() || []));
	});

	function chooseCluster() {
		const index = untrack(selected);
		const clusterz = untrack(clusters);
		if (clusterz !== undefined && index !== undefined)
			controller.setCluster(clusterz[index]);

		props.hide();
	}

	return (
		<Modal.Simple
			{...props}
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Close"
					onClick={props.hide}
				/>,
				<Button
					children="Save"
					onClick={chooseCluster}
				/>,
			]}
			children={(
				<Show
					fallback={<div>Loading...</div>}
					when={clusters !== undefined}
				>
					<Dropdown
						onChange={setSelected}
						selected={selected}
					>
						<For each={clusters()!}>
							{cluster => (
								<Dropdown.Row>{cluster.meta.name}</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</Show>
			)}
			title="Choose a cluster"
		/>
	);
}
