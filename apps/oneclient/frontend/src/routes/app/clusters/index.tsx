import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/clusters/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div>
			<div>Hello "/app/clusters/"!</div>

			<div className="w-px h-screen bg-red-100/10"></div>

			<div>Hello "/app/clusters/"!</div>
		</div>
	);
}
