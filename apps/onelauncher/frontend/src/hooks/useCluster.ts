import type { ClusterModel } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { useEffect, useState } from 'react';

function useRecentCluster() {
	const result = useCommand('getClusters', bindings.core.getClusters);
	const [cluster, setCluster] = useState<ClusterModel | undefined>();

	useEffect(() => {
		if (!result.data)
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
	}, [result.data]);

	return cluster;
}

export default useRecentCluster;
