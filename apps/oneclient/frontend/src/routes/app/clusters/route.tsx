import { AnimatedOutlet, AnimatedOutletProvider } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/clusters')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="flex w-full h-full">
			<BackgroundGradients />

			<AnimatedOutletProvider>
				<AnimatedOutlet
					enter={{
						initial: {
							bottom: '-100%',
							opacity: 0,
						},
						animate: {
							bottom: 0,
							opacity: 1,
						},
					}}
					exit={{
						initial: {
							bottom: 0,
							opacity: 1,
						},
						animate: {
							bottom: '-100%',
							opacity: 0,
						},
					}}
					from={Route.id}
				/>
			</AnimatedOutletProvider>
		</div>
	);
}

function BackgroundGradients() {
	return (
		<div className="relative">
			<div style={{
				background: 'rgba(0, 0, 0, 0.80)',
			}}
			>
			</div>
			<div style={{
				background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, #11171C 60.1%)',
			}}
			>
			</div>
		</div>
	);
}
