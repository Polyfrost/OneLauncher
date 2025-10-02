import ScrollableContainer from '@/components/ScrollableContainer';
import usePopState from '@/hooks/usePopState';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { GameSettings, ProcessSettings } from '../settings/minecraft';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/settings')({
	component: RouteComponent,
});

function RouteComponent() {
	const { id } = Route.useSearch();

	const cluster = useCommand('getClusterById', () => bindings.core.getClusterById(Number(id.toString()) as unknown as bigint));
	const _result = useCommand('getProfileOrDefault', () => bindings.core.getProfileOrDefault(cluster.data?.setting_profile_name as string), {
		enabled: !!cluster.data?.setting_profile_name,
	});

	const save = useCommand('updateClusterProfile', () => bindings.core.updateClusterProfile(cluster.data?.name as string, _result.data), {
		enabled: false,
		subscribed: false,
	});

	usePopState(() => {
		save.refetch();
	});

	if (_result.isPending)
		return <p>loading...</p>;

	return (
		<Sidebar.Page>
			<ScrollableContainer>
				<div className="h-full">
					<h1>Minecraft Settings</h1>

					<GameSettings key={_result.data?.name} settings={_result.data} />

					<ProcessSettings key={_result.data?.name} settings={_result.data} />
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}
