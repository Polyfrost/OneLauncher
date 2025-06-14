import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/test')({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/app/test"!</div>;
}
