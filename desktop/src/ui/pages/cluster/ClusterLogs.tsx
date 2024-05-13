import { useSearchParams } from '@solidjs/router';
import { For, Show, createEffect, createResource, createSignal } from 'solid-js';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { getClusterLog, getClusterLogs } from '../../../bridge/game';

function ClusterLogs() {
	const [params] = useSearchParams();
	const [logs] = createResource(params.id, getClusterLogs);

	const [activeLogFile, setActiveLogFile] = createSignal<string | null>(null);
	const [log, setLog] = createSignal<string | null>(null);

	createEffect(() => {
		const log = logs()?.[0];

		// Set default log to the first file
		if (log && activeLogFile() === null)
			setActiveLogFile(log);

		// Fetch log content
		if (activeLogFile() !== null) {
			getClusterLog(params.id!, activeLogFile()!).then((log) => {
				setLog(log);
			});
		}
	});

	return (
		<div class="flex flex-col flex-1">
			<h1>Logs</h1>

			<div class="flex flex-col flex-1">
				<div class="flex flex-row gap-x-1 overflow-auto">
					<For each={logs() || []}>
						{(log) => {
							const pretty = log.split('/').pop();
							return (
								<button
									class={`border border-gray-10 rounded-md px-2 py-1 ${activeLogFile() === log ? 'bg-gray-10' : ''}`}
									onClick={() => setActiveLogFile(log)}
									children={pretty}
								/>
							);
						}}
					</For>
				</div>

				{/* TODO: Try fix this when using OverlayScrollbarComponent https://github.com/KingSora/OverlayScrollbars/issues/627  */}
				<Show when={log() !== null} fallback={<span>No logs found</span>}>
					<div class="bg-component-bg flex flex-1 h-full font-mono font-medium overflow-auto p-2 rounded-md mt-2">
						<OverlayScrollbarsComponent class="flex-1 h-full relative">
							<code class="whitespace-pre select-text absolute">{log()}</code>
						</OverlayScrollbarsComponent>
					</div>
				</Show>
			</div>
		</div>
	);
}

export default ClusterLogs;
