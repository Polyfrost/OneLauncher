import type { PropsWithChildren } from 'react';
import { GameBackground, LoaderSuspense, Navbar } from '@/components';
import { useCachedImage } from '@/hooks/useCachedImage';
import { useAllClusterDirWatch } from '@/hooks/useClusterDirWatch';
import { useActiveCluster } from '@/hooks/useClusters';
import { bindings } from '@/main';
import { getOnlineClusterForVersion, getOnlineEntryForVersion, getVersionInfoOrDefault } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { motion } from 'motion/react';
import { MouseParallax } from 'react-just-parallax';

export const Route = createFileRoute('/app')({
	component: RouteComponent,
});

function RouteComponent() {
	useAllClusterDirWatch();

	return (
		<LoaderSuspense spinner={{ size: 'large' }}>
			<AppShell>
				<div className="h-full w-full">
					{/* <AnimatedOutlet
						enter={{
							initial: { opacity: 0 },
							animate: { opacity: 1 },
						}}
						exit={{
							initial: { opacity: 1 },
							animate: { opacity: 0 },
						}}
						from={Route.id}
						transition={{ duration: 0.25, bounce: 0.1, power: 0.2, type: 'spring' }}
					/> */}
					<Outlet />
				</div>
			</AppShell>
		</LoaderSuspense>
	);
}

function AppShell({
	children,
}: PropsWithChildren) {
	return (
		<div className="flex flex-col h-full w-full">
			<BackgroundGradient />

			<Navbar />

			<div className="flex flex-col w-full h-full">
				{children}
			</div>
		</div>
	);
}

function BackgroundGradient() {
	const cluster = useActiveCluster();
	const versionInfo = getVersionInfoOrDefault(cluster.mc_version);
	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());

	const entry = getOnlineEntryForVersion(cluster.mc_version, versions);
	const onlineCluster = getOnlineClusterForVersion(cluster.mc_version, versions);
	const artPath = entry?.art ?? onlineCluster?.art;
	const artSrc = useCachedImage(artPath);

	return (
		<div className="relative">
			{/* Linear black gradient: left -> right */}
			<div
				className="absolute top-0 left-0 w-screen h-screen -z-10"
				style={{
					background: 'linear-gradient(270deg, rgba(0, 0, 0, 0.00) 35%, rgba(0, 0, 0, 0.60) 87.5%)',
				}}
			>
			</div>

			{/* Radial black gradient */}
			<div
				className="absolute top-0 left-0 w-screen h-screen -z-10" style={{
					background: 'radial-gradient(48.29% 48.29% at 77.29% 50%, rgba(0, 0, 0, 0.00) 0%, rgba(0, 0, 0, 0.64) 100%)',
				}}
			>
			</div>

			{/* Linear black gradient: bottom -> 200 px up */}
			<div
				className="absolute bottom-0 left-0 w-screen h-50 -z-10" style={{
					background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, rgba(0, 0, 0, 0.68) 60%)',
				}}
			>
			</div>

			<MouseParallax isAbsolutelyPositioned strength={0.01} zIndex={-50}>
				<motion.div
					animate={{
						opacity: 1,
						left: '0',
					}}
					className="relative scale-105"
					initial={{
						opacity: 0,
						left: '-10%',
					}}
					key={(cluster.mc_version) + (cluster.mc_loader)}
				>
					{artSrc
						? (
								<img
									alt=""
									className="absolute left-0 top-0 w-screen h-screen scale-110 object-cover"
									src={artSrc}
								/>
							)
						: (
								<GameBackground
									className="absolute left-0 top-0 w-screen h-screen scale-110"
									name={versionInfo.backgroundName}
								/>
							)}
				</motion.div>
			</MouseParallax>
		</div>
	);
}
