import type { ClusterModel, GameLoader, OnlineClusterManifest } from '@/bindings.gen';
import type { ButtonProps } from 'react-aria-components';
import { GameBackground, LaunchButton } from '@/components';
import { useCachedImage } from '@/hooks/useCachedImage';
import { useActiveCluster, useLastPlayedClusters } from '@/hooks/useClusters';
import { bindings } from '@/main';
import useAppShellStore from '@/stores/appShellStore';
import { prettifyLoader } from '@/utils/loaders';
import { animations } from '@/utils/motion';
import { getOnlineClusterForVersion, getOnlineEntryForVersion, getVersionInfo, getVersionInfoOrDefault } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { DotsGridIcon, Settings04Icon } from '@untitled-theme/icons-react';
import { motion } from 'motion/react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function getCreatedAtMs(createdAt: string): number | null {
	const parsed = Date.parse(createdAt);
	if (Number.isFinite(parsed))
		return parsed;

	// Normalize high-precision RFC3339 fractions for stricter engines.
	const normalized = createdAt.replace(/\.(\d{3})\d+(?=(?:Z|[+-]\d{2}:\d{2})$)/, '.$1');
	const normalizedParsed = Date.parse(normalized);
	return Number.isFinite(normalizedParsed) ? normalizedParsed : null;
}

function getBoostedCreatedAtMs(cluster: ClusterModel, now: number, boostWindowMs: number): number | null {
	if (cluster.last_played !== null)
		return null;

	const createdAtMs = getCreatedAtMs(cluster.created_at);
	if (createdAtMs === null)
		return null;

	return (now - createdAtMs) < boostWindowMs ? createdAtMs : null;
}

function RouteComponent() {
	// Preload the clusters for version page
	useCommandSuspense(['getClustersGroupedByMajor'], bindings.oneclient.getClustersGroupedByMajor);

	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());
	const { data: lastPlayedClusters } = useLastPlayedClusters();

	const setActiveClusterId = useAppShellStore(state => state.setActiveClusterId);
	const activeCluster = useActiveCluster();

	const navigate = useNavigate({
		from: Route.id,
	});

	const BOOST_WINDOW_MS = 7 * 24 * 60 * 60 * 1000;
	const now = Date.now();
	const displayClusters = [...lastPlayedClusters].sort((a, b) => {
		const aBoostedCreatedAtMs = getBoostedCreatedAtMs(a, now, BOOST_WINDOW_MS);
		const bBoostedCreatedAtMs = getBoostedCreatedAtMs(b, now, BOOST_WINDOW_MS);

		if (aBoostedCreatedAtMs !== null && bBoostedCreatedAtMs === null)
			return -1;
		if (bBoostedCreatedAtMs !== null && aBoostedCreatedAtMs === null)
			return 1;
		if (aBoostedCreatedAtMs !== null && bBoostedCreatedAtMs !== null)
			return bBoostedCreatedAtMs - aBoostedCreatedAtMs;

		return 0;
	}).slice(0, 3);

	return (
		<div className="flex h-full w-full flex-col justify-center p-12">
			<ActiveClusterInfo cluster={activeCluster} versions={versions} />

			<motion.div {...animations.slideInUp} className="flex flex-row transition-[height] h-52 gap-6">
				{displayClusters.map(cluster => (
					<RecentsCard
						active={activeCluster.id === cluster.id}
						key={cluster.folder_name}
						loader={cluster.mc_loader}
						onPress={() => setActiveClusterId(cluster.id)}
						version={cluster.mc_version}
						versions={versions}
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

function ActiveClusterInfo({ cluster, versions }: { cluster: ClusterModel; versions: OnlineClusterManifest }) {
	const versionInfo = getVersionInfoOrDefault(cluster.mc_version, versions);
	const entry = getOnlineEntryForVersion(cluster.mc_version, versions);
	const navigate = useNavigate({ from: Route.id });

	const viewCluster = () => {
		navigate({
			to: '/app/cluster/mods',
			search: {
				clusterId: cluster.id,
			},
		});
	};

	const subtitle = entry?.name ?? versionInfo.shortDescription;

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
			key={(cluster.mc_version) + (cluster.mc_loader)}
			transition={{ ease: 'backInOut', duration: 0.35 }}
		>
			<h1 className="text-6xl font-bold text-fg-primary">{cluster.mc_version} {prettifyLoader(cluster.mc_loader)}</h1>
			<p className="text-lg font-medium text-fg-secondary">{subtitle}</p>

			<div className="flex flex-row justify-center items-center gap-2">
				<LaunchButton cluster={cluster} size="large" />

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
	versions: OnlineClusterManifest;
}

function RecentsCard({
	version,
	loader,
	onPress,
	active,
	versions,
}: RecentsCardProps) {
	const versionInfo = getVersionInfo(version, versions);
	const backgroundName = versionInfo?.backgroundName ?? 'MinecraftBuilding';
	const entry = getOnlineEntryForVersion(version, versions);
	const onlineCluster = getOnlineClusterForVersion(version, versions);

	const artPath = entry?.art ?? onlineCluster?.art;
	const artSrc = useCachedImage(artPath);

	if (!versionInfo && !artPath)
		return (
			<Card blur>
				<p className="text-lg font-medium text-fg-secondary">Unknown Version</p>
			</Card>
		);

	return (
		<Card className={twMerge(active && 'outline-2 outline-brand')} onPress={onPress}>
			<div className="flex w-full h-full justify-start items-end px-6 py-3 hover:brightness-80">
				{artSrc
					? (
							<img
								alt={`Minecraft ${version} landscape`}
								className="absolute -z-10 left-0 top-0 w-full h-full object-cover scale-110"
								src={artSrc}
							/>
						)
					: (
							<GameBackground className="absolute -z-10 left-0 top-0 w-full h-full scale-110" name={backgroundName} />
						)}

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
