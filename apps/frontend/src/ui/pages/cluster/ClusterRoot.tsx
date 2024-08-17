import { type Navigator, Route, useIsRouting, useSearchParams } from '@solidjs/router';
import { type ParentProps, Show, createEffect, createResource, createSignal, onCleanup, onMount } from 'solid-js';
import { EyeIcon, File06Icon, Globe04Icon, Image03Icon, PackagePlusIcon, Settings04Icon } from '@untitled-theme/icons-solid';
import type { UnlistenFn } from '@tauri-apps/api/event';
import Sidebar from '../../components/Sidebar';
import AnimatedRoutes from '../../components/AnimatedRoutes';
import ErrorBoundary from '../../components/ErrorBoundary';
import ClusterOverview from './ClusterOverview';
import ClusterLogs from './ClusterLogs';
import ClusterMods from './ClusterMods';
import ClusterScreenshots from './ClusterScreenshots';
import ClusterSettings from './ClusterSettings';
import ClusterGame from './ClusterGame';
import ClusterWorlds from './ClusterWorlds';
import useClusterContext, { ClusterProvider } from '~ui/hooks/useCluster';
import { supportsMods } from '~utils';
import { tryResult } from '~ui/hooks/useCommand';
import { bridge } from '~imports';
import PlayerHead from '~ui/components/game/PlayerHead';

function ClusterRoutes() {
	return (
		<>
			<Route path="/" component={ClusterOverview} />
			<Route path="/logs" component={ClusterLogs} />
			<Route path="/mods" component={ClusterMods} />
			<Route path="/screenshots" component={ClusterScreenshots} />
			<Route path="/worlds" component={ClusterWorlds} />
			<Route path="/settings" component={ClusterSettings} />
			<Route
				path="/game"
				component={() => {
					const [searchParams] = useSearchParams();

					return (
						<Show keyed when={searchParams.process_uuid}>
							<ClusterGame />
						</Show>
					);
				}}
			/>
		</>
	);
}

function ClusterRoot(props: ParentProps) {
	const [searchParams] = useSearchParams();

	return (
		<ClusterProvider uuid={searchParams.id}>
			<div class="h-full flex flex-1 flex-row gap-x-7">
				<div class="mt-8">
					<ClusterSidebar />
				</div>

				<div class="h-full w-full flex flex-col">
					<AnimatedRoutes>
						<ErrorBoundary>
							{props.children}
						</ErrorBoundary>
					</AnimatedRoutes>
				</div>
			</div>
		</ClusterProvider>
	);
}

ClusterRoot.open = function (navigate: Navigator, uuid: string) {
	navigate(`/clusters/?id=${uuid}`);
};

function ClusterSidebar() {
	const [cluster, { refetch: refetchCluster }] = useClusterContext();
	const [listener, setListener] = createSignal<UnlistenFn>();
	const [runningProcesses, { refetch: refetchProcesses }] = createResource([], async () => {
		const c = cluster();
		if (!c || typeof c.path !== 'string')
			return [];

		return tryResult(bridge.commands.getProcessesDetailedByPath, c.path);
	});

	const isRouting = useIsRouting();

	createEffect(() => {
		if (isRouting())
			refetchCluster();
	});

	onMount(async () => {
		const unlisten = await bridge.events.processPayload.listen(({ payload }) => {
			if (payload.event === 'started' || payload.event === 'finished')
				refetchProcesses();
		});

		setListener(() => unlisten);
	});

	onCleanup(() => {
		listener?.()?.();
	});

	return (
		<Sidebar
			base="/clusters"
			links={{
				Cluster: [
					[<EyeIcon />, 'Overview', '/'],
					(supportsMods(cluster?.()) ? [<PackagePlusIcon />, 'Mods', '/mods'] : undefined),
					[<Image03Icon />, 'Screenshots', '/screenshots'],
					[<Globe04Icon />, 'Worlds', '/worlds'],
					[<File06Icon />, 'Logs', '/logs'],
					[<Settings04Icon />, 'Game Settings', '/settings'],
				],
				...(runningProcesses() && runningProcesses()!.length > 0
					? {
							Running: runningProcesses()!.map((details, index) => {
								const icon = <PlayerHead uuid={details.user} />; ;

								return [icon, `Process #${index + 1}`, '/game', ClusterGame.buildUrl(cluster()!.uuid, details)];
							}),
						}
					: {}),
			}}
		/>
	);
}

ClusterRoot.Routes = ClusterRoutes;

export default ClusterRoot;
