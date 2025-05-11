import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/app/browser/"!</div>;
}
