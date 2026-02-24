import type { ClusterModel, ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { DownloadModsRef, ModCardContextApi, ModWithBundle, onClickOnMod } from '@/components';
import type { StrippedCluster } from '@/routes/onboarding/preferences/version';
import { BundleModListModal, DownloadMods, ModCardContext, Overlay } from '@/components';
import { bindings } from '@/main';
import { OnboardingNavigation } from '@/routes/onboarding/route';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { DotsVerticalIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useImperativeHandle, useMemo, useRef, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/onboarding/preferences/versionCategory')({
	component: RouteComponent,
});

export interface BundleData {
	bundles: Array<ModpackArchive>;
	art: string;
	modsInfo: Array<ModpackFile>;
	clusterId: number;
}

function HandleCLuster(cluster: ClusterModel): Array<ModpackArchive> {
	const { data: bundle } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));
	return bundle;
}

function RouteComponent() {
	const selectedClusters: Array<StrippedCluster> = JSON.parse(localStorage.getItem('selectedClusters') ?? '[]');

	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());
	const bundleQueries = clusters.map(cluster => HandleCLuster(cluster));
	const bundlesData: Record<string, BundleData> = {};
	clusters.forEach((cluster, i) => {
		const selected = selectedClusters.some(selectedCluster => selectedCluster.mc_version === cluster.mc_version && selectedCluster.mc_loader === cluster.mc_loader);
		if (!selected)
			return;
		const version = versions.clusters.find(version => cluster.mc_version.startsWith(`1.${version.major_version}`));
		bundlesData[cluster.name] = {
			bundles: bundleQueries[i],
			art: version?.art ?? '/versions/art/Horse_Update.jpg',
			modsInfo: [],
			clusterId: cluster.id,
		};
	});

	const [modsPerCluster, setModsPerCluster] = useState<Record<string, Array<ModWithBundle>>>(
		clusters.reduce((acc, cluster, i) => {
			const bundles = bundleQueries[i];
			const enabledMods = bundles
				.filter(bundle => bundle.manifest.enabled)
				.flatMap(bundle => bundle.manifest.files
					.filter(file => file.enabled)
					.map(file => ({ file, bundleName: bundle.manifest.name })));

			acc[cluster.id] = enabledMods;
			return acc;
		}, {} as Record<string, Array<ModWithBundle>>),
	);

	const bundlesPerCluster = useMemo(() => {
		const result: Record<number, Array<ModpackArchive>> = {};
		clusters.forEach((cluster, i) => {
			const selected = selectedClusters.some(sc => sc.mc_version === cluster.mc_version && sc.mc_loader === cluster.mc_loader);
			if (selected)
				result[cluster.id] = bundleQueries[i];
		});
		return result;
	}, [clusters, bundleQueries, selectedClusters]);

	const downloadModsRef = useRef<DownloadModsRef>(null);
	const wrappedRef = useRef<DownloadModsRef>(null);

	useImperativeHandle(wrappedRef, () => ({
		async openDownloadDialog(nextPath?: string) {
			// Extract overrides from all enabled bundles before downloading mods
			for (const [clusterId, bundles] of Object.entries(bundlesPerCluster)) {
				for (const bundle of bundles) {
					if (bundle.manifest.enabled) {
						try {
							await bindings.oneclient.extractBundleOverrides(bundle.path, Number(clusterId));
						}
						catch (e) {
							console.error(`Failed to extract overrides for bundle ${bundle.manifest.name}:`, e);
						}
					}
				}
			}

			// Then delegate to the actual mod download dialog
			downloadModsRef.current?.openDownloadDialog(nextPath);
		},
	}), [bundlesPerCluster]);

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
			<OnboardingNavigation disableNext={selectedClusters.length === 0} ref={wrappedRef} />
		</>
	);
}

function ModCategory({ bundleData, name, modsPerCluster, setModsPerCluster }: { bundleData: BundleData; name: string; modsPerCluster: Record<string, Array<ModWithBundle>>; setModsPerCluster: React.Dispatch<React.SetStateAction<Record<string, Array<ModWithBundle>>>> }) {
	return (
		<div>
			<h1 className="text-3xl font-semibold my-2">{name}</h1>
			<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-2 gap-6">
				{bundleData.bundles.map(bundle => (
					<ModCategoryCard
						art={bundleData.art}
						bundle={bundle}
						clusterId={bundleData.clusterId}
						fullVersionName={bundle.manifest.name.match(/\[(.*?)\]/)?.[1] ?? 'Loading...'}
						key={bundle.manifest.name}
						mods={modsPerCluster[bundleData.clusterId]}
						setMods={newMods => setModsPerCluster(prev => ({ ...prev, [bundleData.clusterId]: typeof newMods === 'function' ? newMods(prev[bundleData.clusterId]) : newMods }))}
					/>
				))}
			</div>
		</div>
	);
}

function ModCategoryCard({ art, fullVersionName, bundle, mods, setMods, clusterId }: { fullVersionName: string; art: string; bundle: ModpackArchive; mods: Array<ModWithBundle>; setMods: React.Dispatch<React.SetStateAction<Array<ModWithBundle>>>; clusterId: number }) {
	const files = bundle.manifest.files;
	const enabledFiles = files.filter(file => file.enabled);
	const hiddenEnabledFiles = enabledFiles.filter(file => file.hidden);
	const visibleEnabledFiles = enabledFiles.filter(file => !file.hidden);
	const bundleName = bundle.manifest.name;

	const isSelected = visibleEnabledFiles.length > 0
		&& visibleEnabledFiles.every(file => mods.some(m => m.file === file && m.bundleName === bundleName));

	const handleDownload = () => {
		setMods((prevMods) => {
			if (isSelected) {
				return prevMods.filter(mod => mod.bundleName !== bundleName || !enabledFiles.includes(mod.file));
			}
			else {
				const filesToAdd = enabledFiles
					.filter(file => !prevMods.some(m => m.file === file && m.bundleName === bundleName))
					.map(file => ({ file, bundleName }));
				return [...filesToAdd, ...prevMods];
			}
		});
	};

	const onClickOnMod: onClickOnMod = (file) => {
		setMods((prevMods) => {
			const existingIndex = prevMods.findIndex(m => m.file === file && m.bundleName === bundleName);
			const nextMods = existingIndex >= 0
				? prevMods.filter((_, i) => i !== existingIndex)
				: [{ file, bundleName }, ...prevMods];

			const hasVisibleSelection = nextMods.some(mod => (
				mod.bundleName === bundleName
				&& mod.file.enabled
				&& !mod.file.hidden
			));

			if (!hasVisibleSelection)
				return nextMods.filter(mod => mod.bundleName !== bundleName || !hiddenEnabledFiles.includes(mod.file));

			const hiddenToAdd = hiddenEnabledFiles
				.filter(hiddenFile => !nextMods.some(mod => mod.file === hiddenFile && mod.bundleName === bundleName))
				.map(hiddenFile => ({ file: hiddenFile, bundleName }));

			return [...hiddenToAdd, ...nextMods];
		});
	};

	const modsForContext = mods.filter(m => !m.file.hidden).map(m => m.file);

	const context = useMemo<ModCardContextApi>(() => ({
		onClickOnMod,
		useVerticalGridLayout: true,
		mods: modsForContext,
	}), [modsForContext]);

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
						{visibleEnabledFiles.length} Mods {isSelected ? 'Selected' : ''}
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
