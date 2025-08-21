import { bindings } from '@/main';
import useAppShellStore from '@/stores/appShellStore';
import { useCommandSuspense } from '@onelauncher/common';

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

	return clusters.find(c => c.id === clusterId) ?? clusters.at(0);
}
