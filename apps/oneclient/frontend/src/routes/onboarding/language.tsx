import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/language')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div>
			Hello "/onboarding/language"!
		</div>
	);
}
