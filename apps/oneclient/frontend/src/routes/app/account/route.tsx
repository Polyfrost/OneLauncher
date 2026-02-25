import type { MinecraftCredentials } from '@/bindings.gen';
import { LoaderSuspense } from '@/components';
import { bindings } from '@/main';
import { MissingAccountData } from '@/routes/app/account/skins';
import { createFileRoute, Outlet } from '@tanstack/react-router';

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
		const { profile, search: validSearch } = search as AccountsRouteSearchParams;

		if (!profile)
			return { profileData: null, profile: null, validSearch, playerData: null };

		const query = context.queryClient.ensureQueryData({
			queryKey: ['fetchLoggedInProfile', profile.access_token],
			queryFn: () => bindings.core.fetchLoggedInProfile(profile.access_token),
		});

		const profileData = await query;

		const playerDataQuery = context.queryClient.ensureQueryData({
			queryKey: ['fetchMinecraftProfile', profileData.id],
			queryFn: () => bindings.core.fetchMinecraftProfile(profileData.id),
		});

		const playerData = await playerDataQuery;

		return {
			profileData,
			profile,
			validSearch,
			playerData,
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
