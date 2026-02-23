import type { ModCardContextApi } from '@/components';
import { ModCardContext, ModList } from '@/components';
import { useCustomBundle } from '@/hooks/useCustomBundle';
import { getFilePackageType } from '@/routes/app/cluster/mods';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useMemo } from 'react';

export const Route = createFileRoute('/app/cluster/resource-packs')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));
	const { data: installedPackages } = useCommandSuspense(['getLinkedPackages', cluster.id], () => bindings.core.getLinkedPackages(cluster.id));

	const filteredBundles = useMemo(() =>
		bundles.map(b => ({
			...b,
			manifest: { ...b.manifest, files: b.manifest.files.filter(f => getFilePackageType(f) === 'resourcepack') },
		})), [bundles]);

	const customBundle = useCustomBundle(bundles, installedPackages, cluster, 'resourcepack');

	const allBundles = useMemo(() =>
		customBundle !== null ? [...filteredBundles, customBundle] : filteredBundles,
	[filteredBundles, customBundle]);

	const customTogglePaths = useMemo(() => new Set(['__custom__']), []);

	const context = useMemo<ModCardContextApi>(() => ({
		enableClickToDownload: true,
		installedPackages,
	}), [installedPackages]);

	if (allBundles.every(b => b.manifest.files.length === 0))
		return <p className="p-4 text-fg-secondary">No resource packs found in {cluster.name}</p>;

	return (
		<OverlayScrollbarsComponent className="bg-none">
			<div className="min-h-148">
				<ModCardContext.Provider value={context}>
					<ModList bundles={allBundles} cluster={cluster} toggleBundlePaths={customTogglePaths} />
				</ModCardContext.Provider>
			</div>
		</OverlayScrollbarsComponent>
	);
}
