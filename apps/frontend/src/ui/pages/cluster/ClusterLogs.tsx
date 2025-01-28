import { open } from '@tauri-apps/plugin-shell';
import { LinkExternal01Icon, Upload01Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import FormattedLog from '~ui/components/content/FormattedLog';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand, { tryResult } from '~ui/hooks/useCommand';
import useNotifications from '~ui/hooks/useNotifications.tsx';
import useSettings from '~ui/hooks/useSettings';
import { join } from 'pathe';
import { createEffect, createSignal, For, Show, untrack } from 'solid-js';

function ClusterLogs() {
	const [cluster] = useClusterContext();
	const [logs] = useCommand(() => bridge.commands.getClusterLogs(cluster()!.uuid));
	const { settings } = useSettings();
	const notifications = useNotifications();

	const [activeLogFile, setActiveLogFile] = createSignal<string | null>(null);
	const [logContent, setLogContent] = createSignal<string | null>(null);

	function getAndSetLog(log_name: string) {
		tryResult(() => bridge.commands.getClusterLog(cluster()!.uuid, log_name)).then(setLogContent);
	}

	function changeLog(index: number) {
		const log = untrack(() => logs());
		if (log === undefined || log[index] === undefined)
			return;

		setActiveLogFile(log[index]);
	}

	async function openFolder() {
		const root = settings().config_dir;
		const path = cluster()!.path;
		if (root === null || root === undefined || path === null || path === undefined)
			return;

		const dir = join(root, 'clusters', path);
		open(join(dir, 'logs'));
	}

	async function uploadAndOpenLog() {
		const log = activeLogFile();
		if (log === null)
			return;

		const id = await tryResult(() => bridge.commands.uploadLog(cluster()!.uuid, log));

		open(`https://mclo.gs/${id}`).then(() => notifications.set('logs', {
			title: 'Log Uploaded',
			message: 'Opening in your browser.',
		}));
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
		<div class="flex flex-1 flex-col">
			<div class="flex flex-1 flex-col gap-y-2">
				<div class="h-10 flex flex-row items-center justify-between gap-x-1">
					<h1>Logs</h1>
					<div class="flex flex-row gap-x-2">
						<Button
							buttonStyle="secondary"
							children="Upload"
							disabled={missingLogs()}
							iconLeft={<Upload01Icon />}
							onClick={uploadAndOpenLog}
						/>

						<Dropdown
							class="min-w-50"
							disabled={missingLogs()}
							onChange={changeLog}
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
							children="Open Folder"
							iconLeft={<LinkExternal01Icon />}
							onClick={openFolder}
						/>
					</div>
				</div>

				<Show fallback={<span>No logs were found.</span>} when={logContent() !== null}>
					<FormattedLog log={logContent()!} />
				</Show>
			</div>
		</div>
	);
}

export default ClusterLogs;
