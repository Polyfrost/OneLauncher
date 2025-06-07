import ScrollableContainer from '@/components/ScrollableContainer';
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
	const result = useCommand('getProfileOrDefault', () => bindings.core.getProfileOrDefault(cluster.data?.name as string), {
		enabled: !!cluster.data?.name,
	});

	return (
		<Sidebar.Page>
			<ScrollableContainer>
				<div className="h-full">
					<h1>Minecraft Settings</h1>

					<GameSettings />

					<ProcessSettings />
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}
