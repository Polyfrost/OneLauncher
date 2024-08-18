import { Route, useNavigate } from '@solidjs/router';
import { type Accessor, type Context, type ParentProps, type Setter, createContext, createSignal, onMount, useContext } from 'solid-js';
import { createStore } from 'solid-js/store';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserPackage from './BrowserPackage';
import type { Cluster, ManagedPackage, Package, PackageType, Providers } from '~bindings';
import { tryResult } from '~ui/hooks/useCommand';
import { bridge } from '~imports';

interface BrowserControllerType {
	cluster: Accessor<Cluster | undefined>;
	setCluster: Setter<Cluster | undefined>;

	displayBrowser: (cluster?: Cluster | undefined) => void;
	displayPackage: (id: string, provider: Providers) => void;
	displayCategory: (category: string) => void;

	refreshCache: () => void;
	cache: () => PackageCacheType;
};

const BrowserContext = createContext() as Context<BrowserControllerType>;

type PackageCacheType = Partial<{
	[key in PackageType]: ManagedPackage[];
}>;

// Used to cache packages for use on the main browser page.
const [packageCache, setPackageCache] = createStore<PackageCacheType>({});

function BrowserProvider(props: ParentProps) {
	const [cluster, setCluster] = createSignal<Cluster>();
	const navigate = useNavigate();

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

		refreshCache() {
			tryResult(bridge.commands.searchPackages, 'Modrinth', null, null, null, null, null).then((res) => {
				console.log(res);
				setPackageCache((prev) => {
					prev.mod = res;
					return prev;
				});
			});
		},

		cache() {
			return packageCache;
		},
	};

	onMount(() => {
		if (packageCache.mod === undefined)
			controller.refreshCache();
	});

	return (
		<BrowserContext.Provider value={controller}>
			{props.children}
		</BrowserContext.Provider>
	);
}

export function useBrowserController() {
	const controller = useContext(BrowserContext);

	if (!controller)
		throw new Error('useBrowserController should be called inside its BrowserProvider');

	return controller;
}

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
			<Route path="/package" component={BrowserPackage} />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return (
		<BrowserProvider>
			{props.children}
		</BrowserProvider>
	);
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
