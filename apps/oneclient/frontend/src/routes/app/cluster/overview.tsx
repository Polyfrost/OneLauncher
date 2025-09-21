import { SheetPage } from '@/components/SheetPage';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/cluster/overview')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();

	return (
		<SheetPage.Content>
			<div className="flex flex-col gap-4">
				<h1 className="text-2xl font-semibold">Cluster Overview</h1>
				<p>Welcome to the cluster overview page. Here you can find information about your clusters.</p>
				<div style={{ height: '1500px' }}>
					Cluster Name: {cluster.name}
				</div>
				<p>test</p>
				{/* Additional content can be added here */}
			</div>
		</SheetPage.Content>
	);
}
