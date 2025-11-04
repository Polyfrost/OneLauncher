import type { ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { BundleData } from '../version';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueries } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useCallback, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/onboarding/preferences/versions/bundleCategory')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();

	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());

	const bundleQueries = useQueries({
		queries: clusters.map(cluster => ({
			queryKey: ['getBundlesFor', cluster.id],
			queryFn: () => bindings.oneclient.getBundlesFor(cluster.id),
			suspense: true,
		})),
	});

	const bundlesData: Record<string, BundleData> = {};
	clusters.forEach((clusterData, index) => {
		if (clusterData.id !== cluster.id)
			return;
		const version = versions.clusters.find(versionCluster => clusterData.mc_version.startsWith(`1.${versionCluster.major_version}`));
		const bundles = bundleQueries[index].data ?? [];
		// eslint-disable-next-line react-hooks/rules-of-hooks -- TODO: @Kathund Find a better way to do this that isn't useState
		bundlesData[clusterData.name] = { bundles, art: version?.art ?? '/versions/art/Horse_Update.jpg', modsInfo: useState<Array<ModpackFile>>([]), clusterId: clusterData.id };
	});

	const navigate = useNavigate();
	const openModsList = useCallback(() => navigate({ to: `/onboarding/preferences/versions/bundleMods`, search: { mc_version: cluster.mc_version, mc_loader: cluster.mc_loader } }), [cluster, navigate]);

	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<OverlayScrollbarsComponent>
					<div className="h-164">
						<div className="flex flex-row justify-between">
							<h1 className="text-4xl font-semibold mb-2">{cluster.name}</h1>
							<Button className="text-xl font-semibold my-2" onPress={openModsList} size="normal">Open Mods List</Button>
						</div>

						{Object.entries(bundlesData).map(([name, bundleData], index) => <ModCategory bundleData={bundleData} key={index} name={name} />)}
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

function ModCategory({ bundleData, name }: { bundleData: BundleData; name: string }) {
	return (
		<>
			<h1 className="text-2xl font-semibold my-2">{name}</h1>
			<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-2 gap-6">
				{bundleData.bundles.map((bundle, index) => {
					return (
						<ModCategoryCard
							art={bundleData.art}
							bundle={bundle}
							fullVersionName={bundle.manifest.name.match(/\[(.*?)\]/)?.[1] ?? 'LOADING'}
							key={index}
						/>
					);
				})}
			</div>
		</>
	);
}

function ModCategoryCard({ art, fullVersionName, bundle }: { fullVersionName: string; art: string; bundle: ModpackArchive }) {
	const [isSelected, setSelected] = useState<boolean>(false);
	const files = bundle.manifest.files.filter(file => 'Managed' in file.kind);
	const handleDownload = () => {
		setSelected(prev => !prev);
	};

	return (
		<AriaButton className={twMerge('group cursor-pointer w-full rounded-xl transition-[outline] outline-2 hover:outline-brand', isSelected ? 'outline-brand' : 'outline-ghost-overlay')} onPress={handleDownload}>
			<div className="relative w-full">
				<img
					alt={`Minecraft ${fullVersionName} landscape`}
					className={twMerge('w-full rounded-xl h-32 object-cover transition-[filter] group-hover:brightness-100 group-hover:grayscale-0', isSelected ? 'brightness-100 grayscale-0' : 'brightness-70 grayscale-25')}
					src={`https://raw.githubusercontent.com/Polyfrost/DataStorage/refs/heads/main/oneclient${art}`}
				/>

				<div className={twMerge('absolute -top-2 right-3', isSelected ? 'block' : 'hidden group-hover:block')}>
					<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1">
						{files.length} Mods {isSelected ? 'Selected' : ''}
					</div>
				</div>

				<div className="absolute bottom-3 left-3">
					<div className="flex flex-col items-center justify-center">
						<span className="text-white font-bold px-3 py-1 text-xl">{fullVersionName}</span>
					</div>
				</div>

			</div>
		</AriaButton>
	);
}
