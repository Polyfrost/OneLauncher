import { BundleModsList } from '@/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/preferences/versions/bundleMods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();

	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<h1 className="text-4xl font-semibold mb-2">Choose Mods for {cluster.name}</h1>
				<p className="text-slate-400 text-lg mb-2">
					Something something in corporate style fashion about picking your preferred gamemodes and versions and
					optionally loader so that oneclient can pick something for them
				</p>

				<BundleModsList cluster={cluster} />
			</div>
		</div>
	);
}
