import { BundleModsList } from '@/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/cluster/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	return (
		<BundleModsList cluster={cluster} />
	);
}
