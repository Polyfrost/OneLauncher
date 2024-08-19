import { useNavigate } from '@solidjs/router';
import { type Accessor, type Context, For, type ParentProps, type Setter, Show, createContext, createSignal, onMount, untrack, useContext } from 'solid-js';
import useCommand, { tryResult } from './useCommand';
import type { Cluster, ProviderSearchResults, Providers } from '~bindings';
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
	const [selected, setSelected] = createSignal<number | undefined>();
	const [clusters] = useCommand(bridge.commands.getClusters);
	const controller = useBrowser();

	const currentlySelectedCluster = () => {
		const cluster = controller.cluster();
		if (cluster === undefined)
			return 0;

		// TODO: FIx this
		console.log(clusters());
		const index = clusters()?.findIndex((c) => {
			console.log(c.uuid, cluster.uuid, c.uuid === cluster.uuid);
			return c.uuid === cluster.uuid;
		}) || 0;
		if (index === -1)
			return 0;

		return index;
	};

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
				<Dropdown
					onChange={setSelected}
					selected={currentlySelectedCluster()}
				>
					<Show
						when={clusters !== undefined}
						fallback={<div>Loading...</div>}
					>
						<For each={clusters()!}>
							{cluster => (
								<Dropdown.Row>{cluster.meta.name}</Dropdown.Row>
							)}
						</For>
					</Show>
				</Dropdown>
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