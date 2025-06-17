import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/clusters/')({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/app/clusters/"!</div>;
}
