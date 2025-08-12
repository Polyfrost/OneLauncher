import { SheetPage } from '@/components/SheetPage';
import { bindings } from '@/main';
import { createFileRoute } from '@tanstack/react-router';
import { File02Icon, File05Icon } from '@untitled-theme/icons-react';

export const Route = createFileRoute('/app/cluster/logs')({
	component: RouteComponent,
	async beforeLoad(ctx) {
		const cluster = await bindings.core.getClusterById(ctx.search.clusterId as unknown as bigint);
		// TODO: make it use actual data once that's available
		const logFile = {
			fileName: 'latest.log',
			content: await (await fetch('https://api.mclo.gs/1/raw/M1eHlVd')).text(),
		};
		return { cluster, logFile };
	},
});

function RouteComponent() {
	const { logFile } = Route.useRouteContext();

	return (
		<SheetPage.Content>
			<div className="flex flex-col gap-4">
				<div className="flex justify-between items-center">
					<div className="flex items-center gap-2">
						<h2 className="text-xxl font-semibold">Logs</h2>
						<span className="flex font-mono items-center rounded-md bg-component-bg py-1.5 px-2 gap-1">
							<File05Icon width={20} />
							{logFile.fileName}
						</span>
					</div>
				</div>
				<code className="font-mono p-4 bg-component-bg whitespace-pre-line rounded-xl">
					{logFile.content}
				</code>
				{/* Additional content can be added here */}
			</div>
		</SheetPage.Content>
	);
}
