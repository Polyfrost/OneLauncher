import { invoke } from '@tauri-apps/api/core';
import { type UnlistenFn, listen } from '@tauri-apps/api/event';

const _test_cluster = {
	cluster: {
		id: 'd09575a6-7c17-4399-a7a3-a117fffa5420',
		name: 'Placeholder Name',
		createdAt: 1715454720,
		cover: null,
		group: null,
		client: {
			type: 'Vanilla',
		},
	},
	manifest: {
		id: '1.0 cluster',
		manifest: {
			id: '1.8.9',
			javaVersion: {
				majorVersion: 8,
			},
		},
	},
} as Core.ClusterWithManifest;

export async function getCluster(uuid: string): Promise<Core.ClusterWithManifest> {
	return _test_cluster;

	return await invoke<Core.ClusterWithManifest>(
		'plugin:onelauncher|get_cluster',
		{ uuid },
	);
}

export async function getClusterLogs(uuid: string): Promise<string[]> {
	return ['test log'];
	try {
		return await invoke<string[]>(
			'plugin:onelauncher|get_cluster_logs',
			{ uuid },
		);
	}
	catch (e) {
		console.error(e);
		return [];
	}
}

export async function getClusterLog(uuid: string, log: string): Promise<string> {
	return 'test log';
	return await invoke<string>(
		'plugin:onelauncher|get_cluster_log',
		{ uuid, log },
	);
}

export async function getClustersGrouped(): Promise<Map<string, WithIndex<Core.ClusterWithManifest>[]>> {
	const clusters = await getClusters();
	const map = new Map<string, WithIndex<Core.ClusterWithManifest>[]>();

	clusters.forEach((cluster, index) => {
		const groupName = cluster.cluster.group || 'Unnamed';
		const value: WithIndex<Core.ClusterWithManifest> = {
			...cluster,
			index,
		};

		if (map.has(groupName))
			map.get(groupName)!.push(value);
		else
			map.set(groupName, [value]);
	});

	return map;
}

export async function getClusters(): Promise<Core.ClusterWithManifest[]> {
	return [_test_cluster];
	return await invoke<Core.ClusterWithManifest[]>('plugin:onelauncher|get_clusters');
}

export async function createCluster(cluster: Omit<Core.Cluster, 'id' | 'createdAt'> & { version: string }): Promise<void> {
	return await invoke(
		'plugin:onelauncher|create_cluster',
		{
			name: cluster.name,
			version: cluster.version,
			client: cluster.client,
			// cover: cluster.cover,
			// group: cluster.group,
		},
	);
}

export async function refreshClientManager(): Promise<void> {
	return await invoke('plugin:onelauncher|refresh_client_manager');
}

interface LaunchCallbacks {
	on_launch: (pid: number) => any;
	on_stdout: (line: string) => any;
	on_stderr: (line: string) => any;
};

export function launchCluster(uuid: string, callbacks?: LaunchCallbacks): Promise<number> {
	// eslint-disable-next-line no-async-promise-executor
	return new Promise(async (resolve) => {
		let unlisten_stdout: UnlistenFn | undefined;
		let unlisten_stderr: UnlistenFn | undefined;
		let pid: number = -1;

		function guard(passed_pid: number, fn: () => any | undefined) {
			if (passed_pid !== pid)
				return;

			if (fn)
				fn();
		}

		const unlisten_launch = await listen<number>('game:launch', async (e) => {
			pid = e.payload;

			if (callbacks)
				callbacks.on_launch(pid);

			unlisten_launch!();
		});

		const unlisten_exit = await listen<[number, number]>(`game:exit`, e => guard(e.payload[0], () => {
			if (callbacks) {
				unlisten_stdout!();
				unlisten_stderr!();
			}

			unlisten_exit!();
			resolve(e.payload[1]);
		}));

		if (callbacks) {
			unlisten_stdout = await listen<[number, string]>(`game:stdout`, e => guard(
				e.payload[0],
				callbacks.on_stdout(e.payload[1]),
			));

			unlisten_stderr = await listen<[number, string]>(`game:stderr`, e => guard(
				e.payload[0],
				callbacks.on_stderr(e.payload[1]),
			));
		}

		await invoke<number>(
			'plugin:onelauncher|launch_cluster',
			{ uuid },
		);
	});
}
