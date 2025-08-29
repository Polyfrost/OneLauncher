import type { ClusterModel, GameLoader } from '@/bindings.gen';
import type { ButtonProps } from 'react-aria-components';
import { GameBackground } from '@/components';
import { LaunchButton } from '@/components/LaunchButton';
import { useActiveCluster, useLastPlayedClusters } from '@/hooks/useClusters';
import useAppShellStore from '@/stores/appShellStore';
import { prettifyLoader } from '@/utils/loaders';
import { animations } from '@/utils/motion';
import { getVersionInfo, getVersionInfoOrDefault } from '@/utils/versionMap';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { DotsGridIcon, Settings04Icon } from '@untitled-theme/icons-react';
import { motion } from 'motion/react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { data: lastPlayedClusters } = useLastPlayedClusters();

	const setActiveClusterId = useAppShellStore(state => state.setActiveClusterId);
	const activeCluster = useActiveCluster();

	const navigate = useNavigate({
		from: Route.id,
	});

	return (
		<div className="flex h-full w-full flex-col justify-center p-12">
			<ActiveClusterInfo cluster={activeCluster} />

			<motion.div {...animations.slideInUp} className="flex flex-row transition-[height] h-52 gap-6">
				{lastPlayedClusters.slice(0, 3).map(cluster => (
					<RecentsCard
						active={activeCluster?.id === cluster.id}
						key={cluster.folder_name}
						loader={cluster.mc_loader}
						onPress={() => setActiveClusterId(cluster.id)}
						version={cluster.mc_version}
					/>
				))}

				<Card
					blur
					className="flex flex-col justify-center items-center max-w-24"
					onPress={() => navigate({ to: '/app/clusters' })}
				>
					<DotsGridIcon height={48} width={48} />
				</Card>
			</motion.div>
		</div>
	);
}

function ActiveClusterInfo({
	cluster,
}: {
	cluster: ClusterModel | undefined;
}) {
	const versionInfo = getVersionInfoOrDefault(cluster?.mc_version);
	const navigate = useNavigate({
		from: Route.id,
	});

	const viewCluster = () => {
		if (!cluster)
			return;

		navigate({
			to: '/app/cluster/overview',
			search: {
				clusterId: cluster.id,
			},
		});
	};

	return (
		<motion.div
			animate={{
				position: 'relative',
				left: '0',
			}}
			className="flex flex-1 flex-col justify-center items-start gap-2"
			initial={{
				position: 'relative',
				left: '-50%',
			}}
			key={(cluster?.mc_version ?? Math.random()) + (cluster?.mc_loader ?? '')}
			transition={{ ease: 'backInOut', duration: 0.7 }}
		>
			<h1 className="text-6xl font-bold text-fg-primary">{cluster?.mc_version} {prettifyLoader(cluster?.mc_loader ?? 'vanilla')}</h1>
			<p className="text-lg font-medium text-fg-secondary">{versionInfo.shortDescription}</p>

			<div className="flex flex-row justify-center items-center gap-2">
				<LaunchButton clusterId={cluster?.id} size="large" />

				<Button color="ghost" onPress={viewCluster} size="iconLarge">
					<Settings04Icon />
				</Button>
			</div>
		</motion.div>
	);
}

interface RecentsCardProps {
	version: string;
	loader: GameLoader;
	onPress: () => void;
	active: boolean;
}

function RecentsCard({
	version,
	loader,
	onPress,
	active,
}: RecentsCardProps) {
	const versionInfo = getVersionInfo(version);

	if (!versionInfo)
		return (
			<Card blur>
				<p className="text-lg font-medium text-fg-secondary">Unknown Version</p>
			</Card>
		);

	return (
		<Card className={twMerge(active && 'outline-2 outline-brand')} onPress={onPress}>
			<div className="flex w-full h-full justify-start items-end px-6 py-3 hover:brightness-80">
				<GameBackground className="absolute -z-10 left-0 top-0 w-full h-full scale-110" name={versionInfo.backgroundName} />

				<div
					className="absolute top-0 left-0 -z-10 w-full h-full"
					style={{
						background: 'linear-gradient(180deg, rgba(25, 25, 25, 0.00) 24.52%, rgba(17, 17, 21, 0.75) 65%)',
					}}
				>
				</div>

				<h4 className="text-2xl font-semibold">{version} {prettifyLoader(loader)}</h4>
			</div>
		</Card>
	);
}

const BLUR = '30px';
function Card({
	blur = true,
	children,
	className,
	style,
	onPress,
}: {
	blur?: boolean;
	className?: string | undefined;
} & ButtonProps & React.RefAttributes<HTMLButtonElement>) {
	return (
		<AriaButton
			className={twMerge(
				'relative overflow-hidden flex-1 rounded-xl outline outline-component-border',
				blur
					? 'bg-white/5 hover:bg-white/15 active:bg-white/20'
					: 'hover:bg-ghost-overlay-hover active:bg-ghost-overlay-pressed',
				className,
			)}
			onPress={onPress}
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
		</AriaButton>
	);
}
