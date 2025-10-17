import type { VersionInfo } from '@/utils/versionMap';
import type { JSX } from 'react';
import { getVersionInfoOrDefault } from '@/utils/versionMap';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { DotsVerticalIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useCallback, useMemo } from 'react';
import { Button as AriaButton } from 'react-aria-components';

export const Route = createFileRoute('/onboarding/preferences/versions')({
	component: RouteComponent,
});

interface VersionDataEntry {
	minor_version: number;
	loader: string;
	tags: Array<string>;
}

interface VersionDataCluster {
	major_version: number;
	name: string;
	art: string;
	entries: Array<VersionDataEntry>;
}

interface VersionData {
	clusters: Array<VersionDataCluster>;
}

// placeholder
const versions: VersionData = {
	clusters: [
		{
			major_version: 8,
			name: 'Bountiful Update',
			art: '/versions/art/Horse_Update.jpg',
			entries: [
				{
					minor_version: 9,
					loader: 'forge',
					tags: ['Bedwars', 'Nostalgia', 'PvP', 'SkyBlock', 'UHC'],
				},
			],
		},
		{
			major_version: 21,
			name: 'Tricky Trials',
			art: '/versions/art/Tricky_Trials.png',
			entries: [
				{
					minor_version: 1,
					loader: 'fabric',
					tags: ['PvP', 'Survival'],
				},
				{
					minor_version: 8,
					loader: 'fabric',
					tags: ['PvP', 'SkyBlock', 'Survival'],
				},
			],
		},
	],
};

function RouteComponent() {
	const tags = useMemo(() => {
		const result: Record<string, Array<JSX.Element> | undefined> = {};

		versions.clusters.forEach((cluster) => {
			const versionData = getVersionInfoOrDefault(cluster.major_version);

			cluster.entries.forEach((entry, index) => {
				const card = (
					<VersionCard
						cluster={cluster}
						key={`${versionData.prettyName}.${entry.minor_version}-${index}`}
						version={entry}
						versionData={versionData}
					/>
				);

				entry.tags.forEach((tag) => {
					if (result[tag] === undefined)
						result[tag] = [];
					result[tag].push(card);
				});
			});
		});

		return result;
	}, [versions]);

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

						<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
							{versions.clusters.map((cluster) => {
								const versionData = getVersionInfoOrDefault(cluster.major_version);
								return cluster.entries.map((entry, index) => (
									<VersionCard
										cluster={cluster}
										key={`${versionData.prettyName}.${entry.minor_version}-${index}`}
										version={entry}
										versionData={versionData}
									/>
								));
							})}
						</div>

						<RenderTags tags={tags} />
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</div>
	);
}

function VersionCard({ cluster, versionData, version }: { cluster: VersionDataCluster; versionData: VersionInfo; version: VersionDataEntry }) {
	const fullVersionName = `${versionData.prettyName}.${version.minor_version}`;
	const navigate = useNavigate();
	const openModsList = useCallback(() => navigate({ to: `/onboarding/preferences/mod/cluster`, search: { mc_version: fullVersionName, mc_loader: version.loader } }), [fullVersionName, version, navigate]);

	return (
		<AriaButton className="overflow-hidden transition-colors cursor-pointer w-full" onPress={openModsList}>
			<div className="relative w-full">
				<img
					alt={`Minecraft ${versionData.prettyName} landscape`}
					className="w-full rounded-xl h-32 object-cover"
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
					<span className="text-white font-bold px-3 py-1">{fullVersionName}</span>
				</div>
			</div>
		</AriaButton>
	);
}

function RenderTags({ tags }: { tags: Record<string, Array<JSX.Element> | undefined> }) {
	return Object.entries(tags).map((tag, index) => {
		return (
			<div key={index}>
				<h1 className="text-2xl font-semibold my-2">{tag[0]}</h1>
				<div className="bg-page-elevated p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
					{tag[1]}
				</div>
			</div>
		)
	});
}
