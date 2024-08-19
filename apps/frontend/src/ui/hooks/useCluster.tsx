import { type Accessor, type Context, type ParentProps, type ResourceReturn, Show, createContext, createEffect, createSignal, useContext } from 'solid-js';
import { useNavigate } from '@solidjs/router';
import useCommand, { tryResult } from './useCommand';
import type { Cluster } from '~bindings';
import { bridge } from '~imports';
import ClusterGame from '~ui/pages/cluster/ClusterGame';
import Modal, { createModal } from '~ui/components/overlay/Modal';

const ClusterContext = createContext() as Context<ResourceReturn<Cluster>>;

export function getCluster(uuid: string | undefined | null): ResourceReturn<Cluster> | undefined {
	if (typeof uuid !== 'string' || uuid.length === 0)
		return undefined;

	const resource = useCommand(bridge.commands.getCluster, uuid);
	return resource;
}

export function ClusterProvider(props: ParentProps & { uuid: string | undefined }) {
	// eslint-disable-next-line solid/reactivity -- todo
	const cluster = getCluster(props.uuid);

	return (
		<Show when={cluster !== undefined && cluster![0]() !== undefined}>
			<ClusterContext.Provider value={cluster!}>
				{props.children}
			</ClusterContext.Provider>
		</Show>
	);
}

export function useClusterContext() {
	const context = useContext(ClusterContext);

	if (!context)
		throw new Error('useClusterContext should be called inside its ClusterProvider');

	return context;
}

export function useLaunchCluster(uuid: string | Accessor<string | undefined> | (() => string | undefined) | undefined) {
	const navigate = useNavigate();
	const [error, setError] = createSignal<string | undefined>(undefined);

	const getUuid = () => typeof uuid === 'function' ? uuid() : uuid;

	const modal = createModal(props => (
		<Modal.Error
			{...props}
			message={error()}
		/>
	));

	return () => {
		const uuid = getUuid();

		if (uuid === undefined)
			return;

		tryResult(bridge.commands.runCluster, uuid).then((details) => {
			navigate(`/clusters/game?${ClusterGame.buildUrl(uuid, details).toString()}`);
		}).catch((err) => {
			setError(err);
			modal.show();
		});
	};
}

export function useRecentCluster() {
	const [clusters] = useCommand(bridge.commands.getClusters);
	const [cluster, setCluster] = createSignal<Cluster>();

	createEffect(() => {
		// eslint-disable-next-line no-undef-init -- This is fine. I love eslint rule clash though
		let mostRecentCluster: Cluster | undefined = undefined;
		const list = clusters();

		if (list === undefined)
			return;

		for (const cluster of list) {
			if (mostRecentCluster === undefined) {
				mostRecentCluster = cluster;
				continue;
			}

			if (typeof mostRecentCluster.meta.played_at !== 'string' && typeof cluster.meta.played_at === 'string') {
				mostRecentCluster = cluster;
				continue;
			}

			if (typeof mostRecentCluster.meta.played_at === 'string' && typeof cluster.meta.played_at === 'string') {
				const playedAt = new Date(mostRecentCluster.meta.played_at);
				const clusterPlayedAt = new Date(cluster.meta.played_at);

				if (clusterPlayedAt > playedAt)
					mostRecentCluster = cluster;
			}
		}

		setCluster(mostRecentCluster);
	});

	return cluster;
}

export default useClusterContext;
