import type { ClusterModel, ModpackArchive, ModpackFile, OnlineClusterManifest } from '@/bindings.gen';
import type { ModCardContextApi, ModWithBundle, onClickOnMod } from '@/components';
import { ModCardContext, ModList, Overlay } from '@/components';
import { useCachedImage } from '@/hooks/useCachedImage';
import { bindings } from '@/main';
import { OnboardingNavigation } from '@/routes/onboarding/route';
import useDownloadStore from '@/stores/downloadStore';
import { getOnlineClusterForVersion, parseMcVersion } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { DotsVerticalIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/onboarding/preferences/versionCategory')({
	component: RouteComponent,
});

function extractDisplayName(manifestName: string): string {
	return manifestName.match(/\[(.*?)\]/)?.[1] ?? manifestName;
}

function resolveArt(cluster: ClusterModel, versions: OnlineClusterManifest): string {
	const onlineCluster = getOnlineClusterForVersion(cluster.mc_version, versions);
	const parsed = parseMcVersion(cluster.mc_version);
	const entry = parsed?.minor !== undefined
		? onlineCluster?.entries.find(e => e.minor_version === parsed.minor)
		: undefined;
	return entry?.art ?? onlineCluster?.art ?? '/versions/art/Horse_Update.jpg';
}

interface ClusterWithBundles {
	cluster: ClusterModel;
	bundles: Array<ModpackArchive>;
}

interface UnifiedBundle {
	displayName: string;
	art: string;
	modCount: number;
	clustersWithBundle: Array<ClusterWithBundles>;
}

const BUNDLE_CARD_CONTEXT: ModCardContextApi = { useVerticalGridLayout: true };

function HandleCluster(cluster: ClusterModel): Array<ModpackArchive> {
	const { data: bundle } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));
	return bundle;
}

function RouteComponent() {
	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());
	const bundleQueries = clusters.map(cluster => HandleCluster(cluster));

	// Use ALL local clusters — no filter against selectedClusters.
	const allClusters = useMemo<Array<ClusterWithBundles>>(() => {
		return clusters.map((cluster, i) => ({ cluster, bundles: bundleQueries[i] }));
	}, [clusters, bundleQueries]);

	const bundlesPerCluster = useMemo(() => {
		const result: Record<number, Array<ModpackArchive>> = {};
		for (const { cluster, bundles } of allClusters)
			result[cluster.id] = bundles;
		return result;
	}, [allClusters]);

	// Collect all unique bundle names from every cluster.
	const unifiedBundles = useMemo<Array<UnifiedBundle>>(() => {
		const seen: Map<string, UnifiedBundle> = new Map();
		for (const cwb of allClusters)
			for (const archive of cwb.bundles) {
				const name = extractDisplayName(archive.manifest.name);
				if (!seen.has(name)) {
					const visibleEnabled = archive.manifest.files.filter(f => f.enabled && !f.hidden);
					seen.set(name, {
						displayName: name,
						art: resolveArt(cwb.cluster, versions),
						modCount: visibleEnabled.length,
						clustersWithBundle: [],
					});
				}
				seen.get(name)!.clustersWithBundle.push(cwb);
			}
		return [...seen.values()];
	}, [allClusters, versions]);

	useEffect(() => {
		if (!import.meta.env.DEV)
			return;

		const clusterDiagnostics = allClusters.map(({ cluster, bundles }) => ({
			clusterId: cluster.id,
			clusterName: cluster.name,
			mcVersion: cluster.mc_version,
			mcLoader: cluster.mc_loader,
			bundleCount: bundles.length,
			bundles: bundles.map(bundle => ({
				rawName: bundle.manifest.name,
				displayName: extractDisplayName(bundle.manifest.name),
				enabled: bundle.manifest.enabled,
				totalFiles: bundle.manifest.files.length,
				enabledFiles: bundle.manifest.files.filter(file => file.enabled).length,
				visibleEnabledFiles: bundle.manifest.files.filter(file => file.enabled && !file.hidden).length,
			})),
		}));

		console.warn('[onboarding/preferences/versionCategory] bundle discovery diagnostics', {
			clusters: clusterDiagnostics,
			unifiedDisplayNames: unifiedBundles.map(bundle => bundle.displayName),
		});
	}, [allClusters, unifiedBundles]);

	// modsPerCluster is full state, pre-seeded with all enabled mods from all enabled bundles.
	const [modsPerCluster, setModsPerCluster] = useState<Record<string, Array<ModWithBundle>>>(() => {
		return clusters.reduce((acc, cluster, i) => {
			const bundles = bundleQueries[i];
			const enabledMods = bundles
				.filter(bundle => bundle.manifest.enabled)
				.flatMap(bundle => bundle.manifest.files
					.filter(file => file.enabled)
					.map(file => ({ file, bundleName: bundle.manifest.name })));
			acc[cluster.id] = enabledMods;
			return acc;
		}, {} as Record<string, Array<ModWithBundle>>);
	});

	// Card-level toggle: select/deselect all mods for this bundle across every cluster that has it.
	const toggleBundle = (ub: UnifiedBundle) => {
		setModsPerCluster((prev) => {
			const next = { ...prev };
			const firstCluster = ub.clustersWithBundle[0];
			const firstArchive = firstCluster.bundles.find(a => extractDisplayName(a.manifest.name) === ub.displayName);
			if (!firstArchive)
				return next;

			const visibleEnabled = firstArchive.manifest.files.filter(f => f.enabled && !f.hidden);
			const currentMods = prev[firstCluster.cluster.id];
			const isCurrentlySelected = visibleEnabled.length > 0
				&& visibleEnabled.every(f => currentMods.some(m => m.file === f && m.bundleName === firstArchive.manifest.name));

			for (const cwb of ub.clustersWithBundle) {
				const archive = cwb.bundles.find(a => extractDisplayName(a.manifest.name) === ub.displayName);
				if (!archive)
					continue;
				const enabledFiles = archive.manifest.files.filter(f => f.enabled);
				const clusterMods = prev[cwb.cluster.id];
				if (isCurrentlySelected) {
					next[cwb.cluster.id] = clusterMods.filter(m => m.bundleName !== archive.manifest.name || !enabledFiles.includes(m.file));
				}
				else {
					const toAdd = enabledFiles
						.filter(f => !clusterMods.some(m => m.file === f && m.bundleName === archive.manifest.name))
						.map(f => ({ file: f, bundleName: archive.manifest.name }));
					next[cwb.cluster.id] = [...toAdd, ...clusterMods];
				}
			}
			return next;
		});
	};

	const anySelected = Object.values(modsPerCluster).some(mods => mods.length > 0);

	const setDownloadData = useDownloadStore(s => s.setDownloadData);

	const handleBeforeNext = async () => {
		for (const [clusterId, bundles] of Object.entries(bundlesPerCluster))
			for (const bundle of bundles) {
				const selectedMods = modsPerCluster[clusterId] ?? [];
				const hasSelectedBundleContent = selectedMods.some(mod => mod.bundleName === bundle.manifest.name);
				if (!hasSelectedBundleContent)
					continue;
				try {
					await bindings.oneclient.extractBundleOverrides(bundle.path, Number(clusterId));
				}
				catch (e) {
					console.error(`Failed to extract overrides for bundle ${bundle.manifest.name}:`, e);
				}
			}
		setDownloadData(modsPerCluster, bundlesPerCluster);
	};

	return (
		<>
			<div className="min-h-screen px-7">
				<div className="max-w-6xl mx-auto">
					<OverlayScrollbarsComponent>
						<div className="h-164">
							<h1 className="text-4xl font-semibold mb-2">Bundles</h1>
							<p className="text-slate-400 text-lg mb-4">
								Choose which mod bundles to install. Your selection will be applied across all available versions.
							</p>

							<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-2 gap-6">
								{unifiedBundles.map((ub) => {
									const firstCwb = ub.clustersWithBundle[0];
									const firstArchive = firstCwb.bundles.find(a => extractDisplayName(a.manifest.name) === ub.displayName);
									const visibleEnabled = firstArchive?.manifest.files.filter(f => f.enabled && !f.hidden) ?? [];
									const currentMods = modsPerCluster[firstCwb.cluster.id];
									const isSelected = visibleEnabled.length > 0
										&& visibleEnabled.every(f => currentMods.some(m => m.file === f && m.bundleName === firstArchive?.manifest.name));

									return (
										<BundleCard
											art={ub.art}
											clustersWithBundle={ub.clustersWithBundle}
											displayName={ub.displayName}
											isSelected={isSelected}
											key={ub.displayName}
											modCount={ub.modCount}
											modsPerCluster={modsPerCluster}
											onToggle={() => toggleBundle(ub)}
											setModsPerCluster={setModsPerCluster}
										/>
									);
								})}
							</div>
						</div>
					</OverlayScrollbarsComponent>
				</div>
			</div>
			<OnboardingNavigation disableNext={!anySelected} onBeforeNext={handleBeforeNext} />
		</>
	);
}

function BundleCard({ displayName, art, modCount, isSelected, onToggle, clustersWithBundle, modsPerCluster, setModsPerCluster }: {
	displayName: string;
	art: string;
	modCount: number;
	isSelected: boolean;
	onToggle: () => void;
	clustersWithBundle: Array<ClusterWithBundles>;
	modsPerCluster: Record<string, Array<ModWithBundle>>;
	setModsPerCluster: React.Dispatch<React.SetStateAction<Record<string, Array<ModWithBundle>>>>;
}) {
	const artSrc = useCachedImage(art);

	return (
		<AriaButton
			className={twMerge(
				'group cursor-pointer w-full rounded-xl transition-[outline] outline-2 hover:outline-brand',
				isSelected ? 'outline-brand' : 'outline-ghost-overlay',
			)}
			onPress={onToggle}
		>
			<div className="relative w-full">
				{artSrc
					? (
							<img
								alt={`${displayName} bundle`}
								className={twMerge(
									'w-full rounded-xl h-32 object-cover transition-[filter] group-hover:brightness-100 group-hover:grayscale-0',
									isSelected ? 'brightness-100 grayscale-0' : 'brightness-70 grayscale-25',
								)}
								src={artSrc}
							/>
						)
					: (
							<div className={twMerge(
								'w-full rounded-xl h-32 bg-page-elevated transition-[filter] group-hover:brightness-100 group-hover:grayscale-0',
								isSelected ? 'brightness-100 grayscale-0' : 'brightness-70 grayscale-25',
							)}
							/>
						)}

				<div className={twMerge('absolute -top-2 right-3', isSelected ? 'block' : 'hidden group-hover:block')}>
					<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1">
						{modCount}
						{' '}
						Mods
						{' '}
						{isSelected ? 'Selected' : ''}
					</div>
				</div>

				<ModCardContext.Provider value={BUNDLE_CARD_CONTEXT}>
					<Overlay.Trigger>
						<Button
							className="absolute bottom-3 right-3 p-1 transition-colors"
							color="ghost"
							size="icon"
						>
							<DotsVerticalIcon className="w-4 h-4 text-white" />
						</Button>

						<Overlay>
							<BundleDetailModal
								clustersWithBundle={clustersWithBundle}
								displayName={displayName}
								modsPerCluster={modsPerCluster}
								setModsPerCluster={setModsPerCluster}
							/>
						</Overlay>
					</Overlay.Trigger>
				</ModCardContext.Provider>

				<div className="absolute bottom-3 left-3">
					<span className="text-white font-bold px-3 py-1 text-xl drop-shadow-lg">{displayName}</span>
				</div>
			</div>
		</AriaButton>
	);
}

function BundleDetailModal({ displayName, clustersWithBundle, modsPerCluster, setModsPerCluster }: {
	displayName: string;
	clustersWithBundle: Array<ClusterWithBundles>;
	modsPerCluster: Record<string, Array<ModWithBundle>>;
	setModsPerCluster: React.Dispatch<React.SetStateAction<Record<string, Array<ModWithBundle>>>>;
}) {
	const [selectedClusterId, setSelectedClusterId] = useState<number>(clustersWithBundle[0].cluster.id);

	const selected = clustersWithBundle.find(cwb => cwb.cluster.id === selectedClusterId) ?? clustersWithBundle[0];
	const matchingArchive = selected.bundles.find(a => extractDisplayName(a.manifest.name) === displayName);

	const hiddenEnabledFiles: Array<ModpackFile> = useMemo(() => {
		if (!matchingArchive)
			return [];
		return matchingArchive.manifest.files.filter(f => f.enabled && f.hidden);
	}, [matchingArchive]);

	const bundleName = matchingArchive?.manifest.name ?? '';

	const onClickOnModCb: onClickOnMod = useCallback((file: ModpackFile) => {
		setModsPerCluster((prev) => {
			const prevMods = prev[selectedClusterId];
			const existingIndex = prevMods.findIndex(m => m.file === file && m.bundleName === bundleName);
			const nextMods = existingIndex >= 0
				? prevMods.filter((_, i) => i !== existingIndex)
				: [{ file, bundleName }, ...prevMods];

			const hasVisibleSelection = nextMods.some(mod => (
				mod.bundleName === bundleName
				&& mod.file.enabled
				&& !mod.file.hidden
			));

			const synced = !hasVisibleSelection
				? nextMods.filter(mod => mod.bundleName !== bundleName || !hiddenEnabledFiles.includes(mod.file))
				: [
						...hiddenEnabledFiles
							.filter(hf => !nextMods.some(m => m.file === hf && m.bundleName === bundleName))
							.map(hf => ({ file: hf, bundleName })),
						...nextMods,
					];

			return { ...prev, [selectedClusterId]: synced };
		});
	}, [selectedClusterId, bundleName, hiddenEnabledFiles, setModsPerCluster]);

	const visibleMods = useMemo(() => {
		return modsPerCluster[selectedClusterId].filter(m => !m.file.hidden).map(m => m.file);
	}, [modsPerCluster, selectedClusterId]);

	const context = useMemo<ModCardContextApi>(() => ({
		onClickOnMod: onClickOnModCb,
		useVerticalGridLayout: true,
		mods: visibleMods,
	}), [onClickOnModCb, visibleMods]);

	return (
		<Overlay.Dialog className="bg-page items-start flex flex-col min-w-160">
			<Overlay.Title className="px-4">
				Mods for
				{' '}
				<span className="text-brand">{displayName}</span>
			</Overlay.Title>

			{clustersWithBundle.length > 1 && (
				<div className="flex flex-row gap-2 px-4 pb-3 flex-wrap">
					{clustersWithBundle.map(cwb => (
						<AriaButton
							className={twMerge(
								'px-3 py-1 rounded-lg text-sm transition-colors',
								cwb.cluster.id === selectedClusterId
									? 'bg-brand text-white'
									: 'bg-component-bg text-fg-secondary hover:bg-component-bg-hover',
							)}
							key={cwb.cluster.id}
							onPress={() => setSelectedClusterId(cwb.cluster.id)}
						>
							{cwb.cluster.mc_version}
						</AriaButton>
					))}
				</div>
			)}

			<ModCardContext.Provider value={context}>
				<ModList
					bundles={selected.bundles}
					cluster={selected.cluster}
					selectedTab={displayName}
				/>
			</ModCardContext.Provider>

			<div className="w-full flex flex-row gap-6 items-center justify-end px-4 pt-3">
				<p className="font-normal text-lg text-fg-secondary">
					Selected
					{' '}
					{visibleMods.length}
					{' '}
					mods
				</p>
				<Button
					className="px-12"
					color="primary"
					size="large"
					slot="close"
				>
					Confirm
				</Button>
			</div>
		</Overlay.Dialog>
	);
}
