import type { ModCardContextApi } from '@/components/Bundle';
import { ModCardContext, ModList } from '@/components/Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { useMemo } from 'react';

export const Route = createFileRoute('/app/cluster/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	const context = useMemo<ModCardContextApi>(() => ({
		showModDownloadButton: true,
	}), []);

	if (bundles.length === 0)
		return <p>No bundles found {cluster.name}</p>;

	return (
		<ModCardContext.Provider value={context}>
			<ModList bundles={bundles} cluster={cluster} />
		</ModCardContext.Provider>
	);
}
