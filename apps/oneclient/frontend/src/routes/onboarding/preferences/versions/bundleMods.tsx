import { ModList } from '@/components/Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/preferences/versions/bundleMods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	if (bundles.length === 0)
		return (
			<>
				<p>No bundles found {cluster.name}</p>
			</>
		);

	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<h1 className="text-4xl font-semibold mb-2">Choose Mods for {cluster.name}</h1>
				<p className="text-slate-400 text-lg mb-2">
					Something something in corporate style fashion about picking your preferred gamemodes and versions and
					optionally loader so that oneclient can pick something for them
				</p>

				<ModList bundles={bundles} cluster={cluster} />
			</div>
		</div>
	);
}
