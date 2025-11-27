import type { ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { DownloadModsRef } from '@/components';
import type { ModCardContextApi, onClickOnMod } from '@/components/Bundle';
import type { StrippedCLuster } from './version';
import { DownloadMods } from '@/components';
import { ModCardContext } from '@/components/Bundle';
import { BundleModListModal, Overlay } from '@/components/overlay';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { DotsVerticalIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { OnboardingNavigation } from '../route';

export const Route = createFileRoute('/onboarding/preferences/versionCategory')({
	component: RouteComponent,
});

export interface BundleData {
	bundles: Array<ModpackArchive>;
	art: string;
	modsInfo: Array<ModpackFile>;
	clusterId: number;
}

function RouteComponent() {
	const selectedClusters: Array<StrippedCLuster> = JSON.parse(localStorage.getItem('selectedClusters') ?? '[]');

	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());
	const queryClient = useQueryClient();
	const [bundlesData, setBundlesData] = useState<Record<string, BundleData>>({});
	useEffect(() => {
		(async () => {
			const bundleQueries = await Promise.all(
				clusters.map(cluster =>
					queryClient.fetchQuery({
						queryKey: ['getBundlesFor', cluster.id],
						queryFn: async () => bindings.oneclient.getBundlesFor(cluster.id),
					})),
			);

			const data: Record<string, BundleData> = {};
			clusters.forEach((cluster, index) => {
				const selected = selectedClusters.some(strippedCluster => strippedCluster.mc_version === cluster.mc_version && strippedCluster.mc_loader === cluster.mc_loader);
				if (!selected)
					return;

				const version = versions.clusters.find(versionCluster => cluster.mc_version.startsWith(`1.${versionCluster.major_version}`));
				data[cluster.name] = {
					bundles: bundleQueries[index],
					art: version?.art ?? '/versions/art/Horse_Update.jpg',
					modsInfo: [],
					clusterId: cluster.id,
				};
			});
			setBundlesData(data);
		})();
	}, [clusters, queryClient, versions]);

	const [modsPerCluster, setModsPerCluster] = useState<Record<string, Array<ModpackFile>>>(
		clusters.reduce((acc, cluster) => {
			acc[cluster.id] = [];
			return acc;
		}, {} as Record<string, Array<ModpackFile>>),
	);

	const downloadModsRef = useRef<DownloadModsRef>(null);

	return (
		<>
			<div className="min-h-screen px-7">
				<div className="max-w-6xl mx-auto">
					<OverlayScrollbarsComponent>
						<div className="h-164">
							<div className="flex flex-col gap-6">
								{Object.entries(bundlesData).map(([name, bundleData]) => (
									<ModCategory
										bundleData={bundleData}
										key={name}
										modsPerCluster={modsPerCluster}
										name={name}
										setModsPerCluster={setModsPerCluster}
									/>
								))}
							</div>

							<div className="hidden">
								<DownloadMods modsPerCluster={modsPerCluster} ref={downloadModsRef} />
							</div>
						</div>
					</OverlayScrollbarsComponent>
				</div>
			</div>
			<OnboardingNavigation disableNext={selectedClusters.length === 0} ref={downloadModsRef} />
		</>
	);
}

function ModCategory({ bundleData, name, modsPerCluster, setModsPerCluster }: { bundleData: BundleData; name: string; modsPerCluster: Record<string, Array<ModpackFile>>; setModsPerCluster: React.Dispatch<React.SetStateAction<Record<string, Array<ModpackFile>>>> }) {
	return (
		<div>
			<h1 className="text-3xl font-semibold my-2">{name}</h1>
			<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-2 gap-6">
				{bundleData.bundles.map((bundle) => (
					<ModCategoryCard
						art={bundleData.art}
						bundle={bundle}
						clusterId={bundleData.clusterId}
						fullVersionName={bundle.manifest.name.match(/\[(.*?)\]/)?.[1] ?? 'LOADING'}
						key={bundle.manifest.name}
						mods={modsPerCluster[bundleData.clusterId]}
						setMods={newMods => setModsPerCluster(prev => ({ ...prev, [bundleData.clusterId]: typeof newMods === 'function' ? newMods(prev[bundleData.clusterId]) : newMods }))}
					/>
				))}
			</div>
		</div>
	);
}

function ModCategoryCard({ art, fullVersionName, bundle, mods, setMods, clusterId }: { fullVersionName: string; art: string; bundle: ModpackArchive; mods: Array<ModpackFile>; setMods: React.Dispatch<React.SetStateAction<Array<ModpackFile>>>; clusterId: number }) {
	const files = bundle.manifest.files;
	const isSelected = files.every(file => mods.includes(file));
	const handleDownload = () => {
		setMods((prevMods) => {
			if (isSelected)
				return prevMods.filter(mod => !files.includes(mod));
			else
				return [...files, ...prevMods];
		});
	};

	const onClickOnMod: onClickOnMod = (file) => {
		setMods((prevMods) => {
			if (prevMods.includes(file))
				return prevMods.filter(mod => mod !== file);
			else
				return [file, ...prevMods];
		});
	};

	const context = useMemo<ModCardContextApi>(() => ({
		onClickOnMod,
		useVerticalGridLayout: true,
		mods,
	}), [mods]);

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

				<ModCardContext.Provider value={context}>
					<Overlay.Trigger>
						<Button className="absolute bottom-3 right-3 p-1 transition-colors" color="ghost" size="icon">
							<DotsVerticalIcon className="w-4 h-4 text-white" />
						</Button>

						<Overlay>
							<BundleModListModal clusterId={clusterId} name={fullVersionName} />
						</Overlay>
					</Overlay.Trigger>
				</ModCardContext.Provider>

				<div className="absolute bottom-3 left-3">
					<div className="flex flex-col items-center justify-center">
						<span className="text-white font-bold px-3 py-1 text-xl">{fullVersionName}</span>
					</div>
				</div>

			</div>
		</AriaButton>
	);
}
