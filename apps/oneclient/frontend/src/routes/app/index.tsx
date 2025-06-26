import type { ClusterModel } from '@/bindings.gen';
import type { HTMLAttributes } from 'react';
import { GameBackground } from '@/components';
import { bindings } from '@/main';
import { animations, transitions } from '@/utils/motion';
import { getVersionInfo } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { DotsGridIcon, Settings04Icon } from '@untitled-theme/icons-react';
import { motion } from 'motion/react';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { data: clusters } = useCommandSuspense<Array<ClusterModel | undefined>>(
		'getClusters',
		bindings.core.getClusters,
		{
			select: ((data: Array<ClusterModel>) => {
				const sorted = data.sort((a, b) => {
					if (a.last_played && b.last_played)
						return new Date(b.last_played).getTime() - new Date(a.last_played).getTime();
					else if (a.last_played)
						return -1; // a has last_played, b does not
					else if (b.last_played)
						return 1; // b has last_played, a does not

					const aVersion = a.mc_version.replaceAll('.', '');
					const bVersion = b.mc_version.replaceAll('.', '');
					return Number.parseInt(bVersion) - Number.parseInt(aVersion);
				});

				return [
					sorted[0],
					sorted[1],
					undefined,
				];
			}) as (data: Array<ClusterModel | undefined>) => Array<ClusterModel | undefined>,
		},
	);

	return (
		<div className="flex h-full w-full flex-col justify-center p-12">
			<motion.div {...animations.slideInLeft} className="flex flex-1 flex-col justify-center items-start gap-2" transition={{ ...transitions.spring, delay: 0.2 }}>
				<h1 className="text-6xl font-bold text-fg-primary">1.8.9</h1>
				<p className="text-lg font-medium text-fg-secondary">The Bountiful Update</p>

				<div className="flex flex-row justify-center items-center gap-2">
					<Button size="large">Launch</Button>
					<Button color="ghost" size="iconLarge"><Settings04Icon /></Button>
				</div>
			</motion.div>

			<motion.div {...animations.slideInUp} className="flex flex-row transition-[height] h-52 gap-6">
				{clusters.map((cluster, index) => (
					<RecentsCard key={cluster?.folder_name ?? index} version={cluster?.mc_version} />
				))}

				<Card blur className="flex flex-col justify-center items-center max-w-24">
					<DotsGridIcon height={48} width={48} />
				</Card>
			</motion.div>
		</div>
	);
}

interface RecentsCardProps {
	version: string | undefined;
}

const BLUR = '30px';
function RecentsCard({ version }: RecentsCardProps) {
	const versionInfo = getVersionInfo(version);

	return (
		<Card blur={versionInfo === undefined}>
			{versionInfo && (
				<div className="flex w-full h-full justify-start items-end px-6 py-3">
					<GameBackground className="-z-10" name={versionInfo.backgroundName} />

					<div
						className="absolute top-0 left-0 -z-10 w-full h-full"
						style={{
							background: 'linear-gradient(180deg, rgba(25, 25, 25, 0.00) 24.52%, rgba(17, 17, 21, 0.75) 65%)',
						}}
					>
					</div>

					<h4 className="text-3xl font-semibold">{version}</h4>
				</div>
			)}
		</Card>
	);
}

function Card({
	blur = true,
	children,
	className,
	style,
}: {
	blur?: boolean;
	children?: React.ReactNode;
} & HTMLAttributes<HTMLDivElement>) {
	return (
		<div
			className={twMerge(
				'relative overflow-hidden flex-1 rounded-xl inset-ring-2 inset-ring-component-border',
				blur
					? 'bg-white/5 hover:bg-white/15 active:bg-white/20'
					: 'hover:bg-ghost-overlay-hover active:bg-ghost-overlay-pressed',
				className,
			)}
			style={blur
				? {
						// shitty hack because webkit breaks with css variables in its backdrop filter
						backdropFilter: `blur(${BLUR})`,
						WebkitBackdropFilter: `blur(${BLUR})`,
						...style,
					}
				: style}
		>
			{children}
		</div>
	);
}
