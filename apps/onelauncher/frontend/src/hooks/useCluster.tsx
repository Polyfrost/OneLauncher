import type { ClusterModel } from '@/bindings.gen';
import type { ModalProps } from '@/components/overlay/Modal';
import Modal from '@/components/overlay/Modal';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Dropdown } from '@onelauncher/common/components';
import { useEffect, useState } from 'react';
import { Heading } from 'react-aria-components';

export function useRecentCluster() {
	const result = useCommand('getClusters', bindings.core.getClusters);
	const [cluster, setCluster] = useState<ClusterModel | undefined>();

	useEffect(() => {
		if (!result.isSuccess)
			return;

		let mostRecentCluster: ClusterModel | undefined;

		for (const c of result.data) {
			if (!mostRecentCluster) {
				mostRecentCluster = c;
				continue;
			}

			const currentPlayed = mostRecentCluster.last_played;
			const newPlayed = c.last_played;

			if (typeof currentPlayed !== 'string' && typeof newPlayed === 'string') {
				mostRecentCluster = c;
			}
			else if (typeof currentPlayed === 'string' && typeof newPlayed === 'string') {
				const playedAt = new Date(currentPlayed);
				const clusterPlayedAt = new Date(newPlayed);

				if (clusterPlayedAt > playedAt)
					mostRecentCluster = c;
			}
		}

		setCluster(mostRecentCluster);
	}, [result.data, result.isSuccess]);

	return cluster;
}

export function useClusters() {
	const result = useCommand('getClusters', bindings.core.getClusters);
	const [clusters, setClusters] = useState<Array<ClusterModel> | undefined>();

	useEffect(() => {
		if (!result.isSuccess)
			return;
		setClusters(result.data);
	}, [result.data, result.isSuccess]);

	return clusters;
}

type ChooseClusterModalProps = ModalProps & {
	clusters?: Array<ClusterModel>;
	confirmText?: string;
	selected?: (clusters: Array<ClusterModel>) => number;
	onSelected?: (cluster: ClusterModel) => void;
};

export function ChooseClusterModal({ clusters: clusterList, selected: defaultSelected, onSelected, confirmText, ...props }: ChooseClusterModalProps) {
	const clusters = useClusters();
	const [selected, setSelected] = useState(0);

	useEffect(() => {
		if (!clusters)
			return;

		const newSelected = defaultSelected?.(clusters);

		if (newSelected)
			setSelected(newSelected);
	}, [clusters, defaultSelected]);

	function chooseCluster() {
		if (clusters !== undefined) {
			const cluster = clusters[selected] as ClusterModel | undefined;
			if (cluster !== undefined)
				onSelected?.(cluster);
		}
	}

	return (
		<Modal {...props}>
			<div className="min-w-sm flex flex-col rounded-lg bg-page text-center p-4 gap-2">

				<Heading className="text-xl font-semibold" slot="title">Select a Cluster</Heading>
				<Dropdown
					onSelectionChange={(index) => {
						setSelected(index as number);
					}}
					selectedKey={selected}
				>
					{clusters?.map((cluster, index) => (
						<Dropdown.Item id={index} key={cluster.id}>
							{cluster.name}
						</Dropdown.Item>
					))}
				</Dropdown>

				<div className="flex gap-2">
					<Button
						children="Close"
						color="secondary"
						slot="close"
					/>
					<Button
						children={confirmText ?? 'Save'}
						onClick={chooseCluster}
						slot="close"
					/>
				</div>
			</div>
		</Modal>
	);

	// return (
	// 	<Modal
	// 		{...props}
	// 		buttons={[
	// 			<Button
	// 				buttonStyle="secondary"
	// 				children="Close"
	// 				onClick={props.hide}
	// 			/>,
	// 			<Button
	// 				children="Save"
	// 				onClick={chooseCluster}
	// 			/>,
	// 		]}
	// 		children={(
	// 			<Switch>
	// 				<Match when={clusters() !== undefined}>
	// 					<Dropdown
	// 						disabled={clusters()?.length === 0}
	// 						onChange={setSelected}
	// 						selected={selected}
	// 					>
	// 						<Switch>
	// 							<Match when={(clusters()?.length || 0) > 0}>
	// 								<For each={clusters()!}>
	// 									{cluster => (
	// 										<Dropdown.Row>{cluster.meta.name}</Dropdown.Row>
	// 									)}
	// 								</For>
	// 							</Match>
	// 							<Match when>
	// 								<Dropdown.Row>No clusters found</Dropdown.Row>
	// 							</Match>
	// 						</Switch>
	// 					</Dropdown>
	// 				</Match>
	// 				<Match when>
	// 					<div>Loading...</div>
	// 				</Match>
	// 			</Switch>
	// 		)}
	// 		title="Choose a cluster"
	// 	/>
	// );
}
