import { type Params, useNavigate, useSearchParams } from '@solidjs/router';
import { createSignal, onCleanup, onMount } from 'solid-js';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { SlashOctagonIcon } from '@untitled-theme/icons-solid';
import ClusterRoot from './ClusterRoot';
import { bridge } from '~imports';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import { TimeAgo } from '~ui/components/DynamicTime';
import Tooltip from '~ui/components/base/Tooltip';
import FormattedLog from '~ui/components/content/FormattedLog';
import Button from '~ui/components/base/Button';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import type { DetailedProcess } from '~bindings';

interface ClusterGameParams extends Params {
	process_uuid: string;
	started_at: string;
	pid: string;
	user: string;
};

function ClusterGame() {
	const [cluster] = useClusterContext();
	const [params] = useSearchParams<ClusterGameParams>();
	const navigate = useNavigate();

	const [unlisten, setUnlisten] = createSignal<UnlistenFn>();

	onMount(async () => {
		const unlisten = await bridge.events.processPayload.listen(({ payload }) => {
			if (payload.event === 'finished' && payload.uuid === params.process_uuid && cluster() !== undefined)
				ClusterRoot.open(navigate, cluster()!.uuid);
		});

		setUnlisten(() => unlisten);
	});

	onCleanup(() => {
		unlisten()?.();
	});

	const startedAtFormatted = () => {
		const date = new Date(params.started_at!);
		return date.toLocaleString();
	};

	const killModal = createModal(props => (
		<Modal.Delete
			{...props}
			title="Kill Game"
			children="Are you sure you want to kill the game?"
			onDelete={() => killProcess(true)}
			deleteBtnText="Kill $1"
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

	return (
		<Sidebar.Page>
			<h1>Game Running</h1>
			<div class="flex flex-1 flex-col gap-y-4">
				<div class="flex flex-col gap-y-2">
					<div class="flex flex-row">
						<Tooltip text={startedAtFormatted()}>
							Started:
							{' '}
							<strong>
								<TimeAgo timestamp={new Date(params.started_at!).getTime()} />
							</strong>
						</Tooltip>
					</div>

					<p>
						PID:
						{' '}
						<strong>{params.pid || 0}</strong>
					</p>
				</div>

				<FormattedLog log="aaa" />

				<div class="flex flex-row items-center justify-end gap-x-2">
					<Button
						buttonStyle="danger"
						children="Kill"
						iconLeft={<SlashOctagonIcon />}
						onClick={() => killProcess(false)}
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
