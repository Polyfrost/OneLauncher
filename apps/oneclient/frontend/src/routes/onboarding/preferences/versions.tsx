import CavesAndCliffs from '@/assets/backgrounds/CavesAndCliffs.jpg';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { DotsVerticalIcon, PlusIcon } from '@untitled-theme/icons-react';

export const Route = createFileRoute('/onboarding/preferences/versions')({
	component: RouteComponent,
});

// placeholder
const versions = [
	{
		version: '1.8.9',
		tags: ['PvP', 'Skyblock', 'Bedwars', 'UHC'],
	},
	{
		version: '1.21.4',
		tags: ['Nostalgia', 'Survival', 'UHC'],
	},
	{
		version: '1.21.4',
		tags: ['PvP', 'Minigames', 'Survival', 'UHC'],
	},
];

function RouteComponent() {
	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<h1 className="text-4xl font-semibold mb-2">Starting Versions</h1>
				<p className="text-slate-400 text-lg mb-2">
					Something something in corporate style fashion about picking your preferred gamemodes and versions and
					optionally loader so that oneclient can pick something for them
				</p>

				<div className="bg-page-elevated h-full p-4 rounded-xl grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
					{versions.map(version => (
						<VersionCard
							key={version.version}
							tags={version.tags}
							version={version.version}
						/>
					))}
					<div className="border border-component-bg-hover rounded-xl">
						<div className="flex items-center justify-center h-32">
							<PlusIcon className="text-component-bg-hover size-12" />
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}

interface VersionCardProps {
	version: string;
	tags: Array<string>;
}

export function VersionCard({ version, tags }: VersionCardProps) {
	return (
		<div className="overflow-hidden transition-colors cursor-pointer">
			<div className="relative">
				<img
					alt={`Minecraft ${version} landscape`}
					className="w-full rounded-xl h-32 object-cover"
					src={CavesAndCliffs}
				/>

				<div className="absolute top-3 left-3 flex flex-wrap gap-1">
					{tags.map(tag => (
						<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1" key={tag}>
							{tag}
						</div>
					))}
				</div>

				<Button className="absolute bottom-3 right-3 p-1 transition-colors" color="ghost" size="icon">
					<DotsVerticalIcon className="w-4 h-4 text-white" />
				</Button>

				<div className="absolute bottom-3 left-3">
					<span className="text-white font-bold px-3 py-1">{version}</span>
				</div>
			</div>
		</div>
	);
}
