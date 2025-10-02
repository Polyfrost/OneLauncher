import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/preferences/')({
	component: RouteComponent,
});

function RouteComponent() {
	return <div>Hello "/onboarding/preferences/"!</div>;
}
