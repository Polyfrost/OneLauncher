import { useSearchParams } from '@solidjs/router';
import { For, createEffect, createResource, createSignal } from 'solid-js';
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
		<div class="h-full">
			<h1>Logs</h1>

			<div class="flex flex-col h-full">
				<div class="flex flex-row gap-x-1 overflow-auto">
					<For each={logs() || []}>
						{(log) => {
							const pretty = log.split('/').pop();
							return (
								<button
									class={`border border-gray-0.10 rounded-md px-2 py-1 ${activeLogFile() === log ? 'bg-gray-0.10' : ''}`}
									onClick={() => {
										console.log(log);
										setActiveLogFile(log);
									}}
									children={pretty}
								/>
							);
						}}
					</For>
				</div>

				<div class="flex-1 h-full font-mono font-medium overflow-auto">
					<code class="whitespace-pre select-text">{log()}</code>
				</div>
			</div>
		</div>
	);
}

export default ClusterLogs;
