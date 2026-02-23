import type { GameLoader, OnlineCluster, OnlineClusterEntry } from '@/bindings.gen';
import type { VersionInfo } from '@/utils/versionMap';
import { bindings } from '@/main';
import { OnboardingNavigation } from '@/routes/onboarding/route';
import { getVersionInfoOrDefault } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export interface StrippedCluster {
	mc_version: string;
	mc_loader: GameLoader;
}

export const Route = createFileRoute('/onboarding/preferences/version')({
	component: RouteComponent,
});

function RouteComponent() {
	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	// const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());

	const [selectedClusters, setSelectedClusters] = useState<Array<StrippedCluster>>([]);

	return (
		<>
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
									return cluster.entries.map(entry => (
										<VersionCard
											cluster={cluster}
											fullVersionName={`${versionData.prettyName}.${entry.minor_version}`}
											key={`${versionData.prettyName}.${entry.minor_version}-${entry.loader}`}
											setSelectedClusters={setSelectedClusters}
											version={entry}
											versionData={versionData}
										/>
									));
								})}
							</div>

						</div>
					</OverlayScrollbarsComponent>
				</div>
			</div>

			<OnboardingNavigation disableNext={selectedClusters.length === 0} />
		</>
	);
}

function VersionCard({ cluster, versionData, version, fullVersionName, setSelectedClusters }: { cluster: OnlineCluster; versionData: VersionInfo; version: OnlineClusterEntry; fullVersionName: string; setSelectedClusters: React.Dispatch<React.SetStateAction<Array<StrippedCluster>>> }) {
	const [isSelected, setSelected] = useState<boolean>(false);
	const toggle = () => {
		setSelected(prev => !prev);
		setSelectedClusters((prev) => {
			let updatedClusters: Array<StrippedCluster> = [];
			const exists = prev.some(strippedCluster => strippedCluster.mc_version === fullVersionName && strippedCluster.mc_loader === version.loader);
			if (exists)
				updatedClusters = prev.filter(strippedCluster => !(strippedCluster.mc_version === fullVersionName && strippedCluster.mc_loader === version.loader));
			else
				updatedClusters = [...prev, { mc_version: fullVersionName, mc_loader: version.loader }];

			localStorage.setItem('selectedClusters', JSON.stringify(updatedClusters));
			return updatedClusters;
		});
	};

	return (
		<AriaButton className={twMerge('group overflow-hidden cursor-pointer w-full rounded-xl transition-[outline] outline-2 hover:outline-brand', isSelected ? 'outline-brand' : 'outline-ghost-overlay')} onPress={toggle}>
			<div className="relative w-full">
				<img
					alt={`Minecraft ${versionData.prettyName} landscape`}
					className={twMerge('w-full rounded-xl h-32 object-cover transition-[filter] group-hover:brightness-100 group-hover:grayscale-0', isSelected ? 'brightness-100 grayscale-0' : 'brightness-70 grayscale-25')}
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

				<div className="absolute bottom-3 left-3">
					<span className="text-white font-bold px-3 py-1 text-xl">{fullVersionName}</span>
				</div>
			</div>
		</AriaButton>
	);
}
