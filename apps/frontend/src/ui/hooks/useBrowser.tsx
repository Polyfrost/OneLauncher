import { useNavigate } from '@solidjs/router';
import { type Accessor, type Context, For, type ParentProps, type Setter, Show, createContext, createEffect, createSignal, onMount, untrack, useContext } from 'solid-js';
import type { Cluster, ProviderSearchResults, Providers } from '@onelauncher/client/bindings';
import useCommand, { tryResult } from './useCommand';
import { useRecentCluster } from './useCluster';
import { bridge } from '~imports';
import BrowserPackage from '~ui/pages/browser/BrowserPackage';
import type { ModalProps } from '~ui/components/overlay/Modal';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import Dropdown from '~ui/components/base/Dropdown';
import Button from '~ui/components/base/Button';

interface BrowserControllerType {
	cluster: Accessor<Cluster | undefined>;
	setCluster: Setter<Cluster | undefined>;

	displayBrowser: (cluster?: Cluster | undefined) => void;
	displayPackage: (id: string, provider: Providers) => void;
	displayCategory: (category: string) => void;
	displayClusterSelector: () => void;

	refreshCache: () => void;
	cache: Accessor<ProviderSearchResults | undefined>;
};

const BrowserContext = createContext() as Context<BrowserControllerType>;

export function BrowserProvider(props: ParentProps) {
	const [mainPageCache, setMainPageCache] = createSignal<ProviderSearchResults>();

	const [cluster, setCluster] = createSignal<Cluster>();
	const recentCluster = useRecentCluster();
	const navigate = useNavigate();

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

		async refreshCache() {
			const res = await tryResult(bridge.commands.searchPackages, 'Modrinth', null, 10, null, null, null, null);

			setMainPageCache(res);
		},

		cache: mainPageCache,
	};

	onMount(() => {
		if (mainPageCache() === undefined)
			controller.refreshCache();
	});

	createEffect(() => {
		setCluster(recentCluster());
	});

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
	const [clusters] = useCommand(bridge.commands.getClusters);
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
			title="Choose a cluster"
			children={(
				<Show
					when={clusters !== undefined}
					fallback={<div>Loading...</div>}
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
			buttons={[
				<Button
					children="Close"
					onClick={props.hide}
					buttonStyle="secondary"
				/>,
				<Button
					children="Save"
					onClick={chooseCluster}
				/>,
			]}
		/>
	);
}
