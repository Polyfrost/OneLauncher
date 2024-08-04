import { Route, useNavigate } from '@solidjs/router';
import { createContext, createMemo, createResource, createSignal, useContext, type Accessor, type Context, type ParentProps, type Setter } from 'solid-js';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserPackage from './BrowserPackage';
import type { Cluster, ManagedPackage, Providers } from '~bindings';
import { tryResult } from '~ui/hooks/useCommand';
import { bridge } from '~imports';

type BrowserControllerType = {
	cluster: Accessor<Cluster | undefined>;
	setCluster: Setter<Cluster | undefined>;

	displayBrowser: (cluster?: Cluster | undefined) => void;
	displayPackage: (id: string, provider: Providers) => void;
	displayCategory: (category: string) => void;

	// fetchPackages: (provider: Providers) => ManagedPackage[];
}

const BrowserContext = createContext() as Context<BrowserControllerType>;

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
			navigate(BrowserPackage.buildUrl({ id, provider }))
		},

		displayCategory(category: string) {

		},
	};

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
