import { LoaderSuspense, Navbar } from '@/components';
import { GameBackground } from '@/components/GameBackground';
import { AnimatedOutlet } from '@onelauncher/common/components';
import { createFileRoute, Outlet } from '@tanstack/react-router';

export const Route = createFileRoute('/app')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<LoaderSuspense spinner={{ size: 'large' }}>
			<div className="flex flex-col h-full w-full">
				<Navbar />

				<Background />

				<div className="h-full w-full pb-12">
					{/* <AnimatedOutlet
						enter={{
							initial: { opacity: 1, left: '-100%' },
							animate: { opacity: 1, left: 0 },
						}}
						exit={{
							initial: { opacity: 1, left: 0 },
							animate: { opacity: 1, left: '150%' },
						}}
						from={Route.id}
						transition={{ duration: 0.3, bounce: 0.1, power: 0.2, type: 'spring' }}
					/> */}
					<Outlet />
				</div>
			</div>
		</LoaderSuspense>
	);
}

function Background() {
	return (
		<div>
			{/* Linear gradient: left -> right */}
			<div
				className="absolute top-0 left-0 w-screen h-screen -z-10"
				style={{
					background: 'linear-gradient(270deg, rgba(0, 0, 0, 0.00) 35%, rgba(0, 0, 0, 0.60) 87.5%)',
				}}
			>
			</div>

			{/* Radial Gradient */}
			<div
				className="absolute top-0 left-0 w-screen h-screen -z-10" style={{
					background: 'radial-gradient(48.29% 48.29% at 77.29% 50%, rgba(0, 0, 0, 0.00) 0%, rgba(0, 0, 0, 0.64) 100%)',
				}}
			>
			</div>

			{/* Linear gradient: bottom -> 200 px up */}
			<div
				className="absolute bottom-0 left-0 w-screen h-50 -z-10" style={{
					background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, rgba(0, 0, 0, 0.68) 60%)',
				}}
			>
			</div>

			<GameBackground name="HypixelSkyblockHub" />
		</div>
	);
}
