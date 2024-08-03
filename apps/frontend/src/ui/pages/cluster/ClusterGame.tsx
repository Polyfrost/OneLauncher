import { type Params, useNavigate, useSearchParams } from '@solidjs/router';
import { createEffect, createSignal, onCleanup, onMount } from 'solid-js';
import type { UnlistenFn } from '@tauri-apps/api/event';
import ClusterRoot from './ClusterRoot';
import { bridge } from '~imports';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import { TimeAgo } from '~ui/components/DynamicTime';
import Tooltip from '~ui/components/base/Tooltip';
import useCommand from '~ui/hooks/useCommand';
import FormattedLog from '~ui/components/content/FormattedLog';

interface ClusterGameParams extends Params {
	process_uuid: string;
};

function ClusterGame() {
	const [cluster] = useClusterContext();
	const [params] = useSearchParams<ClusterGameParams>();
	const navigate = useNavigate();

	const [unlisten, setUnlisten] = createSignal<UnlistenFn>();
	const [startedAt, setStartedAt] = createSignal<number>();
	const [pid] = useCommand(bridge.commands.getPidByUuid, params.process_uuid || '');

	createEffect(() => {
		if (params.process_uuid === undefined)
			throw new Error('process_uuid is required');

		const c = cluster();
		if (c === undefined)
			throw new Error('Cluster not found');

		if (typeof c.meta.played_at !== 'string')
			setStartedAt(Date.now());
		else
			setStartedAt(new Date(c.meta.played_at).getTime());
	});

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

	const startedAtFormatted = (startedAt: number) => {
		const date = new Date(startedAt);
		return date.toLocaleString();
	};

	return (
		<Sidebar.Page>
			<h1>Game Running</h1>
			<div class="flex flex-col gap-y-2">
				<div class="flex flex-row">
					<Tooltip text={startedAtFormatted(startedAt() || 0)}>
						Started:
						{' '}
						<strong>
							<TimeAgo timestamp={startedAt() || 0} />
						</strong>
					</Tooltip>
				</div>

				<p>
					PID:
					{' '}
					<strong>{pid() || 0}</strong>
				</p>
			</div>

			<FormattedLog log="aaa" />
		</Sidebar.Page>
	);
}

ClusterGame.buildUrl = function (uuid: string, process_uuid: string): URLSearchParams {
	return new URLSearchParams({ id: uuid, process_uuid });
};

export default ClusterGame;
