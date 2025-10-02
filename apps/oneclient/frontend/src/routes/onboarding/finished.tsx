import { createFileRoute, Link } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding/finished')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<Link to="/app">
			<div className="flex flex-col h-full px-12">
				<div>
					<h1 className="text-4xl font-semibold mb-2">Finished!</h1>
					<p className="text-slate-400 text-lg mb-2">Thank you for using OneClient</p>
				</div>
			</div>

			<div className="absolute bottom-2 left-1/2 -translate-x-1/2 flex flex-row gap-2">
				<p className="text-slate-500 text-lg mb-2">Click anywhere to exit</p>
			</div>
		</Link>
	);
}
