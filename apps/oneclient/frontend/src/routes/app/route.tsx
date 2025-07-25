import type { PropsWithChildren } from 'react';
import { LoaderSuspense, Navbar } from '@/components';
import { GameBackground } from '@/components/GameBackground';
import useAppShellStore from '@/stores/appShellStore';
import { AnimatedOutlet } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { MouseParallax } from 'react-just-parallax';

export const Route = createFileRoute('/app')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<LoaderSuspense spinner={{ size: 'large' }}>
			<AppShell>
				<div className="h-full w-full">
					<AnimatedOutlet
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
					/>
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
	const background = useAppShellStore(state => state.background);

	if (background === 'none')
		return undefined;

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
				<GameBackground
					className="absolute left-0 top-0 w-screen h-screen scale-110"
					name="HypixelSkyblockHub"
				/>
			</MouseParallax>
		</div>
	);
}
