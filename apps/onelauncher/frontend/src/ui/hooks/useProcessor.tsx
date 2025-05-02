import type { Cluster, DetailedProcess } from '@onelauncher/client/bindings';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { bridge } from '~imports';
import { type Accessor, createEffect, createSignal, onCleanup, onMount, type Resource } from 'solid-js';
import useCommand, { tryResult } from './useCommand';

interface ProcessorHookType {
	running: Resource<DetailedProcess[]>;
	stop: (id: string) => Promise<unknown>;
	isRunning: Accessor<boolean>;
};

export default function useProcessor(cluster: Cluster): ProcessorHookType {
	const [runningProcesses, { refetch: refetchProcesses }] = useCommand(() => bridge.commands.getProcessesDetailedByPath(cluster.path!));
	const [unlisten, setUnlisten] = createSignal<UnlistenFn>();
	const [isRunning, setIsRunning] = createSignal<boolean>(false);

	onMount(async () => {
		const unlisten = await bridge.events.processPayload.listen((e) => {
			if (e.payload.event === 'started' || e.payload.event === 'finished')
				refetchProcesses();
		});

		setUnlisten(() => unlisten);
	});

	onCleanup(() => {
		unlisten()?.();
	});

	createEffect(() => {
		setIsRunning((runningProcesses()?.length ?? 0) > 0);
	});

	return {
		running: runningProcesses,
		stop: async id => await tryResult(() => bridge.commands.killProcess(id)),
		isRunning,
	};
}
