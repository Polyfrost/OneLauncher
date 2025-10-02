import { SheetPage } from '@/components/SheetPage';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
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
					Cluster Name:
					{' '}
					{cluster.name}

					<TestBundles />
				</div>
				<p>test</p>
				{/* Additional content can be added here */}
			</div>
		</SheetPage.Content>
	);
}

function TestBundles() {
	const { cluster } = Route.useRouteContext();

	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	return (
		<div>
			<h2 className="text-xl font-semibold">Bundles</h2>
			<ul>
				{bundles.map(bundle => (
					<li key={bundle.path}>
						<code>
							{JSON.stringify(bundle, null, 4)}
						</code>
					</li>
				))}
			</ul>
		</div>
	);
}
