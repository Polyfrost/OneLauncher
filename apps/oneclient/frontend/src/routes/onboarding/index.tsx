import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<>
			<div className="absolute top-56 left-32">
				<h2 className="text-6xl font-bold">OneClient</h2>
			</div>
		</>
	);
}
