import { type Params, useSearchParams } from '@solidjs/router';
import { createSignal, onCleanup, onMount } from 'solid-js';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { SlashOctagonIcon } from '@untitled-theme/icons-solid';
import { render } from 'solid-js/web';
import type { DetailedProcess } from '@onelauncher/client/bindings';
import { bridge } from '~imports';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import { TimeAgo } from '~ui/components/DynamicTime';
import Tooltip from '~ui/components/base/Tooltip';
import FormattedLog, { Line } from '~ui/components/content/FormattedLog';
import Button from '~ui/components/base/Button';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import useCommand from '~ui/hooks/useCommand';

interface ClusterGameParams extends Params {
	process_uuid: string;
	started_at: string;
	pid: string;
	user: string;
};

function ClusterGame() {
	const [cluster] = useClusterContext();
	const [params] = useSearchParams<ClusterGameParams>();
	const [log] = useCommand(bridge.commands.getClusterLog, cluster()!.uuid, 'latest.log');
	const [isRunning, setIsRunning] = createSignal(true);

	const [unlisten, setUnlisten] = createSignal<UnlistenFn>();
	let codeRef!: HTMLElement;

	onMount(async () => {
		const unlisten = await bridge.events.processPayload.listen((event) => {
			if (event.payload.uuid !== params.process_uuid)
				return;

			if (event.payload.event === 'logging')
				render(() => <Line line={event.payload.message} />, codeRef);
			else if (event.payload.event === 'finished')
				setIsRunning(false);
		});

		setUnlisten(() => unlisten);
	});

	onCleanup(() => {
		unlisten()?.();
	});

	const killModal = createModal(props => (
		<Modal.Delete
			{...props}
			title="Kill Game"
			children="Are you sure you want to kill the game?"
			onDelete={() => killProcess(true)}
			deleteBtnText="Kill $1"
			timeLeft={1}
		/>
	));

	function killProcess(force: boolean = false) {
		if (params.process_uuid !== undefined) {
			if (force === true) {
				bridge.commands.killProcess(params.process_uuid);
				return;
			}

			killModal.show();
		}
	}

	const date = new Date(params.started_at!);

	return (
		<Sidebar.Page>
			<h1>{isRunning() ? 'Game Running' : 'Game Stopped'}</h1>
			<div class="flex flex-1 flex-col gap-y-4">
				<div class="flex flex-col gap-y-2">
					<div class="flex flex-row">
						<div>
							<Tooltip text={date.toLocaleString()}>
								Started:
								{' '}
								<strong>
									<TimeAgo timestamp={date.getTime()} />
								</strong>
							</Tooltip>
						</div>
					</div>

					<p>
						PID:
						{' '}
						<strong>{params.pid || 0}</strong>
					</p>
				</div>

				<FormattedLog
					codeRef={el => codeRef = el}
					log={log()?.trim()}
					enableAutoScroll={true}
				/>

				<div class="flex flex-row items-center justify-end gap-x-2">
					<Button
						buttonStyle="danger"
						children="Kill"
						iconLeft={<SlashOctagonIcon />}
						onClick={() => killProcess(false)}
						disabled={!isRunning()}
					/>
				</div>
			</div>
		</Sidebar.Page>
	);
}

ClusterGame.buildUrl = function (cluster_id: string, detailed: DetailedProcess): URLSearchParams {
	return new URLSearchParams({
		id: cluster_id,
		process_uuid: detailed.uuid,
		started_at: detailed.started_at,
		pid: detailed.pid.toString(),
		...(detailed.user ? { user: detailed.user } : {}),
	});
};

export default ClusterGame;
