import { bindings } from "@/main";
import useCommand from "./useCommand";
import { useEffect, useState } from "react";
import type { Model } from "@/bindings.gen";

function useRecentCluster() {
	const result = useCommand("getClusters", bindings.commands.getClusters);
	const [cluster, setCluster] = useState<Model | undefined>();

	useEffect(() => {
		if (!result.data) return;

		let mostRecentCluster: Model | undefined;

		for (const c of result.data) {
			if (!mostRecentCluster) {
				mostRecentCluster = c;
				continue;
			}

			const currentPlayed = mostRecentCluster.last_played;
			const newPlayed = c.last_played;

			if (typeof currentPlayed !== 'string' && typeof newPlayed === 'string') {
				mostRecentCluster = c;
			} else if (typeof currentPlayed === 'string' && typeof newPlayed === 'string') {
				const playedAt = new Date(currentPlayed);
				const clusterPlayedAt = new Date(newPlayed);

				if (clusterPlayedAt > playedAt) {
					mostRecentCluster = c;
				}
			}
		}

		setCluster(mostRecentCluster);
	}, [result.data]);

	return cluster;
}

export default useRecentCluster;