import type { ModCardContextApi } from '@/components';
import { ModCardContext, ModList } from '@/components';
import { getFilePackageType } from '@/routes/app/cluster/mods';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useMemo } from 'react';

export const Route = createFileRoute('/app/cluster/shaders')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));
	const { data: installedPackages } = useCommandSuspense(['getLinkedPackages', cluster.id], () => bindings.core.getLinkedPackages(cluster.id));

	const filteredBundles = useMemo(() =>
		bundles.map(b => ({
			...b,
			manifest: { ...b.manifest, files: b.manifest.files.filter(f => getFilePackageType(f) === 'shader') },
		})), [bundles]);

	const context = useMemo<ModCardContextApi>(() => ({
		enableClickToDownload: true,
		installedPackages,
	}), [installedPackages]);

	if (filteredBundles.every(b => b.manifest.files.length === 0))
		return <p className="p-4 text-fg-secondary">No shaders found in {cluster.name}</p>;

	return (
		<OverlayScrollbarsComponent className="bg-none">
			<div className="min-h-148">
				<ModCardContext.Provider value={context}>
					<ModList bundles={filteredBundles} cluster={cluster} />
				</ModCardContext.Provider>
			</div>
		</OverlayScrollbarsComponent>
	);
}
