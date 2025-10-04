import type { MinecraftCredentials } from '@/bindings.gen';
import { LoaderSuspense } from '@/components';
import { bindings } from '@/main';
import { createFileRoute, Outlet } from '@tanstack/react-router';

export interface AccountsRouteSearchParams {
	profile: MinecraftCredentials;
}

export const Route = createFileRoute('/app/account')({
	component: RouteComponent,
	validateSearch: (search): AccountsRouteSearchParams => {
		return {
			profile: search.profile as MinecraftCredentials,
		};
	},
	async beforeLoad({ context, search }) {
		const query = context.queryClient.ensureQueryData({
			queryKey: ['fetchLoggedInProfile', search.profile.access_token],
			queryFn: () => bindings.core.fetchLoggedInProfile(search.profile.access_token),
		});

		const profileData = await query;
		return {
			profileData,
			profile: search.profile,
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
