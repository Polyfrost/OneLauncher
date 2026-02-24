import { bindings } from '@/main';
import useAppShellStore from '@/stores/appShellStore';
import { useAsyncEffect, useCommand, useCommandSuspense } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

export function useLastPlayedClusters() {
	return useCommandSuspense(
		['getClusters', 'sortedByLastPlayed'],
		bindings.core.getClusters,
		{
			select: data => data
				.sort((a, b) => {
					if (a.last_played && b.last_played)
						return new Date(b.last_played).getTime() - new Date(a.last_played).getTime();
					else if (a.last_played)
						return -1; // a has last_played, b does not
					else if (b.last_played)
						return 1; // b has last_played, a does not

					const aVersion = a.mc_version.replaceAll('.', '');
					const bVersion = b.mc_version.replaceAll('.', '');
					return Number.parseInt(bVersion) - Number.parseInt(aVersion);
				}),
		},
	);
}

export function useActiveCluster() {
	const clusterId = useAppShellStore(state => state.activeClusterId);

	const { data: clusters } = useLastPlayedClusters();

	return clusters.find(c => c.id === clusterId) ?? clusters[0];
}

export function useIsRunning(clusterId: number | undefined | null) {
	const queryClient = useQueryClient();
	const enabled = clusterId !== null && clusterId !== undefined;
	const queryClusterId = clusterId ?? -1;
	const { data: running } = useCommand(
		['isClusterRunning', queryClusterId],
		() => bindings.core.isClusterRunning(queryClusterId),
		{
			enabled,
			staleTime: 0,
			gcTime: 0,
			refetchOnWindowFocus: true,
			refetchInterval: 2000,
			refetchIntervalInBackground: true,
		},
	);

	useEffect(() => {
		if (!enabled)
			return;

		// Keep state accurate when the game is force-closed and a process event
		// is delayed/missed.
		const interval = window.setInterval(() => {
			queryClient.invalidateQueries({
				queryKey: ['isClusterRunning', queryClusterId],
			});
		}, 2000);

		return () => {
			window.clearInterval(interval);
		};
	}, [enabled, queryClient, queryClusterId]);

	useAsyncEffect(async () => {
		if (!enabled)
			return;

		const unlisten = await bindings.events.process.on((e) => {
			if (e.cluster_id !== queryClusterId)
				return;

			const isRunning = e.kind.type !== 'Stopped';
			queryClient.setQueryData(['isClusterRunning', queryClusterId], isRunning);
		});

		return () => {
			unlisten();
		};
	}, [enabled, queryClient, queryClusterId]);

	return enabled ? Boolean(running) : false;
}
