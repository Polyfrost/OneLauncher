import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="flex h-full w-full flex-col justify-center">
			<div className="flex flex-1 flex-col justify-center">
				<h1 className="text-5xl font-semibold text-fg-primary">1.8.9</h1>
			</div>

			<div className="flex flex-row h-56">
				<div className="flex-1 rounded-xl border border-solid border-component-border bg-page-overlay">

				</div>
			</div>
		</div>
	);
}
