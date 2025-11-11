import type { MinecraftCredentials } from '@/bindings.gen';
import { LoaderSuspense } from '@/components';
import { bindings } from '@/main';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { MissingAccountData } from './skins';

export interface AccountsRouteSearchParams {
	profile: MinecraftCredentials | undefined;
	search: boolean;
}

export const Route = createFileRoute('/app/account')({
	component: RouteComponent,
	validateSearch: (search): AccountsRouteSearchParams => {
		return {
			profile: search.profile as MinecraftCredentials | undefined,
			search: search.profile === undefined,
		};
	},
	async beforeLoad({ context, search }) {
		// TODO instead replace this with a refresh access_token function. Waiting on binding from @LynithDev
		const { profile, search: validSearch } = search;
		if (!profile)
			return { profileData: null, profile: null, validSearch };

		const query = context.queryClient.ensureQueryData({
			queryKey: ['fetchLoggedInProfile', profile.access_token],
			queryFn: () => bindings.core.fetchLoggedInProfile(profile.access_token),
		});

		const profileData = await query;
		return {
			profileData,
			profile,
			validSearch,
		};
	},
});

function RouteComponent() {
	const { profileData, validSearch } = Route.useRouteContext();

	return (
		<>
			<LoaderSuspense spinner={{ size: 'large' }}>
				{profileData === null ? <MissingAccountData validSearch={validSearch} /> : <Outlet />}
			</LoaderSuspense>
		</>
	);
}
