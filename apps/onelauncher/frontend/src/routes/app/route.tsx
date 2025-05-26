import { AnimatedOutlet } from '@/components/AnimatedOutlet';
import Navbar from '@/components/Navbar';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

export const Route = createFileRoute('/app')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<>
			<div className="flex flex-col px-4">
				<Navbar />
			</div>

			<div className="flex-1 overflow-hidden">
				<OverlayScrollbarsComponent
					className="h-full w-full overflow-auto overflow-x-hidden"
				>
					<div className="h-full pb-4 px-4">

						<AnimatedOutlet
							enter={{
								initial: { opacity: 0, transform: 'translateX(-20%)' },
								animate: { opacity: 1, transform: 'translateX(0)' },
							}}
							exit={{
								initial: { opacity: 1, transform: 'translateX(0)' },
								animate: { opacity: 0, transform: 'translateX(-20%)' },
							}}
							from={Route.id}
							transition={{ duration: 0.3, bounce: 0.1, power: 0.2, type: 'spring' }}
						/>

					</div>
				</OverlayScrollbarsComponent>
			</div>
		</>
	);
}
