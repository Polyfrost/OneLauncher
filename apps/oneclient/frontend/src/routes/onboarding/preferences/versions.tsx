import type { ModpackArchive, ModpackFile, OnlineCluster, OnlineClusterEntry } from '@/bindings.gen';
import type { VersionInfo } from '@/utils/versionMap';
import { bindings } from '@/main';
import { getVersionInfoOrDefault } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueries } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { DotsVerticalIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useCallback, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/onboarding/preferences/versions')({
	component: RouteComponent,
});

interface BundleData {
	bundles: Array<ModpackArchive>;
	art: string;
}

function RouteComponent() {
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
	clusters.forEach((cluster, index) => {
		const version = versions.clusters.find(versionCluster => cluster.mc_version.startsWith(`1.${versionCluster.major_version}`));
		const bundles = bundleQueries[index].data ?? [];
		bundlesData[cluster.name] = { bundles, art: version?.art ?? '/versions/art/Horse_Update.jpg' };
	});

	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<OverlayScrollbarsComponent>
					<div className="h-164">
						<h1 className="text-4xl font-semibold mb-2">Starting Versions</h1>
						<p className="text-slate-400 text-lg mb-2">
							Something something in corporate style fashion about picking your preferred gamemodes and versions and
							optionally loader so that oneclient can pick something for them
						</p>

						<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-2 gap-6">
							{versions.clusters.map((cluster) => {
								const versionData = getVersionInfoOrDefault(cluster.major_version);
								return cluster.entries.map((entry, index) => (
									<VersionCard
										cluster={cluster}
										fullVersionName={`${versionData.prettyName}.${entry.minor_version}`}
										key={`${versionData.prettyName}.${entry.minor_version}-${index}`}
										version={entry}
										versionData={versionData}
									/>
								));
							})}
						</div>

						{Object.entries(bundlesData).map(([name, bundleData], index) => <ModCategory bundleData={bundleData} key={index} name={name} />)}

					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

function VersionCard({ cluster, versionData, version, fullVersionName }: { cluster: OnlineCluster; versionData: VersionInfo; version: OnlineClusterEntry; fullVersionName: string }) {
	const navigate = useNavigate();
	const openModsList = useCallback(() => navigate({ to: `/onboarding/preferences/mod/cluster`, search: { mc_version: fullVersionName, mc_loader: version.loader } }), [fullVersionName, version, navigate]);

	return (
		<AriaButton className="group overflow-hidden cursor-pointer w-full rounded-xl transition-[outline] outline-2 outline-ghost-overlay hover:outline-brand" onPress={openModsList}>
			<div className="relative w-full">
				<img
					alt={`Minecraft ${versionData.prettyName} landscape`}
					className="w-full rounded-xl h-32 object-cover transition-[filter] brightness-70 grayscale-25 group-hover:brightness-100 group-hover:grayscale-0"
					src={`https://raw.githubusercontent.com/PolyFrost/DataStorage/refs/heads/main/oneclient${cluster.art}`}
				/>

				<div className="absolute top-3 left-3 flex flex-wrap gap-1">
					{
						version.tags.map(tag => (
							<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1" key={tag}>
								{tag}
							</div>
						))
					}
				</div>

				<Button className="absolute bottom-3 right-3 p-1 transition-colors" color="ghost" size="icon">
					<DotsVerticalIcon className="w-4 h-4 text-white" />
				</Button>

				<div className="absolute bottom-3 left-3">
					<span className="text-white font-bold px-3 py-1 text-xl">{fullVersionName}</span>
				</div>
			</div>
		</AriaButton>
	);
}

function ModCategory({ name, bundleData }: { name: string; bundleData: BundleData }) {
	const [, setMods] = useState<Array<ModpackFile>>([]);
	return (
		<>
			<h1 className="text-2xl font-semibold my-2">{name}</h1>
			<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
				{bundleData.bundles.map((bundle, index) => {
					return (
						<FunnyVersionCard
							art={bundleData.art}
							bundle={bundle}
							fullVersionName={bundle.manifest.name.match(/\[(.*?)\]/)?.[1] ?? 'LOADING'}
							key={index}
							setMods={setMods}
						/>
					);
				})}
			</div>
		</>
	);
}

function FunnyVersionCard({ art, fullVersionName, setMods, bundle }: { fullVersionName: string; art: string; setMods: React.Dispatch<React.SetStateAction<Array<ModpackFile>>>; bundle: ModpackArchive }) {
	const [isSelected, setSelected] = useState<boolean>(false);
	const files = bundle.manifest.files.filter(file => 'Managed' in file.kind);
	const handleDownload = () => {
		setMods((prevMods) => {
			if (isSelected)
				return prevMods.filter(mod => !files.includes(mod));
			else
				return [...files, ...prevMods];
		});
		setSelected(prev => !prev);
	};

	return (
		<AriaButton className={twMerge('group cursor-pointer w-full rounded-xl transition-[outline] outline-2 hover:outline-brand', isSelected ? 'outline-brand' : 'outline-ghost-overlay')} onPress={handleDownload}>
			<div className="relative w-full">
				<img
					alt="fuck"
					className={twMerge('w-full rounded-xl h-16 object-cover transition-[filter] group-hover:brightness-100 group-hover:grayscale-0', isSelected ? 'brightness-100 grayscale-0' : 'brightness-70 grayscale-25')}
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
