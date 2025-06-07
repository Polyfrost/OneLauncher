import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/complete')({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/onboarding/complete"!</div>;
}
