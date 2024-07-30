import { For, Show, createEffect, createSignal, untrack } from 'solid-js';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { LinkExternal01Icon } from '@untitled-theme/icons-solid';
import { open } from '@tauri-apps/plugin-shell';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import { bridge } from '~imports';
import useClusterContext from '~ui/hooks/useCluster';
import Dropdown from '~ui/components/base/Dropdown';
import Button from '~ui/components/base/Button';
import useSettingsContext from '~ui/hooks/useSettings';
import joinPath from '~utils/helpers';

function ClusterLogs() {
	const [cluster] = useClusterContext();
	const [logs] = useCommand(bridge.commands.getClusterLogs, cluster()!.uuid);
	const settings = useSettingsContext();

	const [activeLogFile, setActiveLogFile] = createSignal<string | null>(null);
	const [log, setLog] = createSignal<string | null>(null);

	function getAndSetLog(log_name: string) {
		tryResult(bridge.commands.getClusterLog, cluster()!.uuid, log_name).then(setLog);
	}

	function changeLog(index: number) {
		const log = untrack(() => logs());
		if (log === undefined || log[index] === undefined)
			return;

		setActiveLogFile(log[index]);
	}

	async function openFolder() {
		const root = settings.config_dir;
		const path = cluster()!.path;
		if (root === null || root === undefined || path === null || path === undefined)
			return;

		const dir = joinPath(root, 'clusters', path);

		open(joinPath(dir, 'logs'));
	}

	const missingLogs = () => logs() === undefined || logs()?.length === 0 || false;

	createEffect(() => {
		const log = logs()?.[0];

		// Set default log to the first file
		if (log !== undefined && activeLogFile() === null)
			setActiveLogFile(log);

		// Fetch log content
		if (activeLogFile() !== null)
			getAndSetLog(activeLogFile()!);
	});

	return (
		<div class="flex flex-col flex-1">
			<div class="flex flex-col flex-1 gap-y-2">
				<div class="flex flex-row justify-between items-center gap-x-1 h-10">
					<h1>Logs</h1>
					<div class="flex flex-row gap-x-2">
						<Dropdown
							onChange={changeLog}
							class="min-w-50"
							disabled={missingLogs()}
						>
							<For each={logs() || ['None']}>
								{(log) => {
									const pretty = log.split('/').pop();
									return (
										<Dropdown.Row>
											<div>
												{pretty}
											</div>
										</Dropdown.Row>
									);
								}}
							</For>
						</Dropdown>
						<Button
							buttonStyle="primary"
							onClick={openFolder}
							children="Open Folder"
							iconLeft={<LinkExternal01Icon />}
							disabled={missingLogs()}
						/>
					</div>
				</div>

				<Show when={log() !== null} fallback={<span>No logs were found.</span>}>
					<div class="bg-component-bg flex flex-1 h-full font-mono font-medium overflow-auto p-2 rounded-md mt-2">
						<OverlayScrollbarsComponent class="flex-1 h-full relative">
							<code class="whitespace-pre select-text absolute line-height-normal">{log()}</code>
						</OverlayScrollbarsComponent>
					</div>
				</Show>
			</div>
		</div>
	);
}

export default ClusterLogs;
