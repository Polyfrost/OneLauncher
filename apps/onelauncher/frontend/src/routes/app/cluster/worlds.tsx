import ScrollableContainer from '@/components/ScrollableContainer';
import { createFileRoute } from '@tanstack/react-router';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/worlds')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<Sidebar.Page>
			<h1>Worlds</h1>
			<ScrollableContainer>
				<div className="h-full">

				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}
