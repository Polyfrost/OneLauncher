import { AnimatedOutlet } from '@/components/AnimatedOutlet';
import Navbar from '@/components/Navbar';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

export const Route = createFileRoute('/app')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="h-full flex flex-col">
			<div className="flex flex-col px-4">
				<Navbar />
			</div>

			<div className="h-full w-full overflow-hidden">
				<div className="relative h-full w-full flex flex-col overflow-x-hidden pb-8">
					<OverlayScrollbarsComponent className="h-full w-full flex flex-col overflow-x-hidden overflow-y-auto">
						<div className="h-full flex-1 px-4">

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
			</div>
		</div>
	);
}
