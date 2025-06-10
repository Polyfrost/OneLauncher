import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/summary')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="grid grid-cols-2 h-full w-full flex-col items-start justify-center gap-x-16 gap-y-2">
			<h1 className="text-6xl">
				Prepare OneLauncher
			</h1>

			<h3>Are you sure you want to proceed with the following tasks?</h3>

			<div>
				<pre>tasks</pre>
			</div>
		</div>
	);
}
