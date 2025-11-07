import { ModList } from '@/components/Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/cluster/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	if (bundles.length === 0)
		return <p>No bundles found {cluster.name}</p>;

	return (
		<ModList bundles={bundles} cluster={cluster} showModDownload />
	);
}
