import ScrollableContainer from '@/components/ScrollableContainer';
import { Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/screenshots')({
	component: RouteComponent,
});

function RouteComponent() {
	const arr = [1, 2, 3, 4, 5, 6];
	return (
		<Sidebar.Page>
			<h1>Screenshots</h1>
			<ScrollableContainer>
				<div className="h-full">
					<Show
						fallback={<div className="text-border/400">No screenshots found. Press F2 in game to take a screenshot!</div>}
						when
					>
						<div className="grid grid-cols-[repeat(auto-fill,minmax(350px,1fr))] w-full transform-gpu gap-2">
							{arr.map(data => (
								<ScreenshotEntry cluster_path="cluster" key={data} path={`screenshot-${data}.png`} />
							))}
						</div>
					</Show>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

function ScreenshotEntry({
	path,
}: {
	path: string;
	cluster_path: string;
}) {
	function onClick() {
		// screen shot viewer overlay
	}

	return (
		<div
			className="flex flex-col items-center gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover hover:opacity-80"
			onClick={onClick}
		>
			<img alt={path} className="aspect-video w-full rounded-lg" src="https://github.com/emirsassan.png" />
		</div>
	);
}
