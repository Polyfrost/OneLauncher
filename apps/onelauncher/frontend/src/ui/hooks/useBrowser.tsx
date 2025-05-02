import type { Cluster, FeaturedPackage, PackageType, Providers, ProviderSearchQuery, SearchResult } from '@onelauncher/client/bindings';
import { useNavigate } from '@solidjs/router';
import { bridge } from '~imports';
import { createModal } from '~ui/components/overlay/Modal';
import BrowserPackage from '~ui/pages/browser/BrowserPackage';
import { PROVIDERS } from '~utils';
import { type Accessor, type Context, createContext, createEffect, createResource, createSignal, on, onMount, type ParentProps, type Resource, type Setter, useContext } from 'solid-js';
import { ChooseClusterModal, useRecentCluster } from './useCluster';
import { tryResult } from './useCommand';

export type ProviderSearchOptions = ProviderSearchQuery & { provider: Providers };
export type PopularPackages = Record<Providers, SearchResult[]>;

interface BrowserControllerType {
	cluster: Accessor<Cluster | undefined>;
	setCluster: Setter<Cluster | undefined>;

	displayBrowser: (cluster?: Cluster | undefined) => void;
	displayPackage: (id: string, provider: Providers) => void;
	displayClusterSelector: () => void;

	search: () => void;
	searchQuery: Accessor<ProviderSearchOptions>;
	setSearchQuery: Setter<ProviderSearchOptions>;

	packageType: Accessor<PackageType>;
	setPackageType: Setter<PackageType>;

	popularPackages: Accessor<PopularPackages | undefined>;
	featuredPackage: Resource<FeaturedPackage[]>;
};

const BrowserContext = createContext() as Context<BrowserControllerType>;

export function BrowserProvider(props: ParentProps) {
	const [popularPackages, setPopularPackages] = createSignal<PopularPackages | undefined>();
	const [featuredPackage] = createResource(async () => {
		try {
			return await tryResult(bridge.commands.getFeaturedPackages);
		}
		catch (e) {
			console.error('Failed to fetch featured packages', e);
			return [];
		}
	});

	// Used for the current "Browser Mode". It'll only show packages of the selected type
	const [packageType, setPackageType] = createSignal<PackageType>('mod');

	// Active cluster that results should be "focused" on
	const [cluster, setCluster] = createSignal<Cluster>();

	const [searchOptions, setSearchOptions] = createSignal<ProviderSearchOptions>({
		provider: 'Modrinth',
		query: '',
		limit: 18, // Even multiples of 3 recommended for grid view (normally 3 columns, 2 columns on very small windows)
		offset: 0,
		categories: null,
		game_versions: null,
		loaders: null,
		package_types: ['mod'],
		open_source: null,
	});

	const navigate = useNavigate();
	const recentCluster = useRecentCluster();

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

	const chooseCluster = (cluster: Cluster) => {
		controller.setCluster(cluster);
	};

	const modal = createModal(props => (
		<ChooseClusterModal
			onSelected={chooseCluster}
			selected={list => currentlySelectedCluster(list) || 0}
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

		displayClusterSelector() {
			modal.show();
		},

		search: () => navigate('/browser/search'),
		searchQuery: searchOptions,
		setSearchQuery: setSearchOptions,

		packageType,
		setPackageType,

		popularPackages,
		featuredPackage,
	};

	onMount(async () => {
		const getOpts = (provider: Providers): ProviderSearchOptions => ({
			provider,
			query: '',
			limit: 10,
			offset: 0,
			categories: null,
			game_versions: null,
			loaders: null,
			package_types: ['mod'],
			open_source: null,
		});

		const response = await Promise.allSettled(
			PROVIDERS.map(provider => tryResult(() => bridge.commands.searchProviderPackages(provider, getOpts(provider)))),
		);

		const popularPackages = response.reduce((acc, res, i) => {
			const provider = PROVIDERS[i] as Providers;

			if (res.status === 'fulfilled')
				acc[provider] = res.value.results;

			return acc;
		}, {} as PopularPackages);

		setPopularPackages(popularPackages);
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
