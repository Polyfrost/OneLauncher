import { LoaderSuspense } from '@/components';
import { bindings } from '@/main';
import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';

export interface ModRouteSearchParams {
	mc_version: string;
	mc_loader: string;
}

export const Route = createFileRoute('/onboarding/preferences/mod')({
	component: RouteComponent,
	validateSearch: (search): ModRouteSearchParams => {
		return {
			mc_version: search.mc_version as string,
			mc_loader: search.mc_loader as string,
		};
	},
	async beforeLoad({ context, search }) {
		if (!search.mc_version)
			throw redirect({ to: '/onboarding/preferences/versions', from: '/onboarding/preferences/mod' });
		if (!search.mc_loader)
			throw redirect({ to: '/onboarding/preferences/versions', from: '/onboarding/preferences/mod' });

		const query = context.queryClient.ensureQueryData({
			queryKey: ['getClusters'],
			queryFn: () => bindings.core.getClusters(),
		});

		const clusters = await query
		const cluster = clusters.find((cluster) => cluster.mc_version === search.mc_version && cluster.mc_loader === search.mc_loader)
		if (!cluster)
			throw redirect({ to: '/onboarding/preferences/versions', from: '/onboarding/preferences/mod' });

		return {
			cluster,
		};
	},
});

function RouteComponent() {
	return (
		<>
			<LoaderSuspense spinner={{ size: 'large' }}>
				<Outlet />
			</LoaderSuspense>

		</>
	);
}
