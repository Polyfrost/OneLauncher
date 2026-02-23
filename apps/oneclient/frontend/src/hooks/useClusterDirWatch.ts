import { useLastPlayedClusters } from '@/hooks/useClusters';
import { bindings } from '@/main';
import { useQueryClient } from '@tanstack/react-query';
import { watch } from '@tauri-apps/plugin-fs';
import { useEffect } from 'react';

/**
 * Watches ALL cluster directories for file-system changes (files added,
 * removed, renamed). Should be mounted once at the app root so that syncing
 * happens regardless of which page the user is on.
 */
export function useAllClusterDirWatch() {
	const { data: clusters } = useLastPlayedClusters();
	const queryClient = useQueryClient();

	useEffect(() => {
		if (clusters.length === 0)
			return;

		const unwatchers: Array<() => void> = [];

		for (const cluster of clusters)
			bindings.folders.fromCluster(cluster.folder_name).then((clusterDir) => {
				watch(
					clusterDir,
					async () => {
						try {
							await bindings.core.syncCluster(cluster.id);
							await queryClient.invalidateQueries({ queryKey: ['getLinkedPackages', cluster.id] });
						}
						catch (e) {
							console.error('[watch] sync failed for cluster', cluster.id, ':', e);
						}
					},
					{ delayMs: 500, recursive: true },
				)
					.then(fn => unwatchers.push(fn))
					.catch(e => console.error('[watch] failed to register watch:', e));
			}).catch(e => console.error('[watch] failed to resolve cluster dir:', e));

		return () => {
			for (const unwatch of unwatchers)
				unwatch();
		};
	}, [clusters, queryClient]);
}
