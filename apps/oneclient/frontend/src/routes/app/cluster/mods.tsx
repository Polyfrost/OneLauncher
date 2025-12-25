import type { ModCardContextApi } from '@/components';
import { ModCardContext, ModList } from '@/components';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useMemo } from 'react';

export const Route = createFileRoute('/app/cluster/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));
	const { data: installedPackages } = useCommandSuspense(['getLinkedPackages', cluster.id], () => bindings.core.getLinkedPackages(cluster.id));

	const context = useMemo<ModCardContextApi>(() => ({
		enableClickToDownload: true,
		installedPackages,
	}), [installedPackages]);

	if (bundles.length === 0)
		return <p>No bundles found {cluster.name}</p>;

	return (
		<OverlayScrollbarsComponent className="bg-none">
			<div className="min-h-148">
				<ModCardContext.Provider value={context}>
					<ModList bundles={bundles} cluster={cluster} />
				</ModCardContext.Provider>
			</div>
		</OverlayScrollbarsComponent>
	);
}
