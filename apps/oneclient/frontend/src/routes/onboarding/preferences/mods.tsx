import { BundleModsList } from '@/components';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

export const Route = createFileRoute('/onboarding/preferences/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { data: clusters } = useCommandSuspense(['getClusters'], bindings.core.getClusters);

	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<h1 className="text-4xl font-semibold mb-2">Choose Mods</h1>
				<p className="text-slate-400 text-lg mb-2">
					Something something in corporate style fashion about picking your preferred gamemodes and versions and
					optionally loader so that oneclient can pick something for them
				</p>

				<Tabs defaultValue={String(clusters[0].id)}>
					<TabList className="gap-6">
						{clusters.map(cluster => <Tab key={cluster.id} value={String(cluster.id)}>{cluster.name}</Tab>)}
					</TabList>

					<TabContent>
						{clusters.map(cluster => (
							<TabPanel key={cluster.id} value={String(cluster.id)}>
								<OverlayScrollbarsComponent>
									<BundleModsList cluster={cluster} />
								</OverlayScrollbarsComponent>
							</TabPanel>
						))}
					</TabContent>
				</Tabs>
			</div>
		</div>
	);
}
