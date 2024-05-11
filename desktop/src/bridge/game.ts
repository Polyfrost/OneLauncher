import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export async function getCluster(uuid: string): Promise<Core.ClusterWithManifest> {
	return await invoke<Core.ClusterWithManifest>(
		'plugin:onelauncher|get_cluster',
		{ uuid },
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

export async function launchCluster(uuid: string, callbacks: LaunchCallbacks): Promise<number> {
	const unlisten_launch = await listen<number>('game:launch', e => callbacks.on_launch(e.payload));
	const unlisten_stdout = await listen<string>('game:stdout', e => callbacks.on_stdout(e.payload));
	const unlisten_stderr = await listen<string>('game:stderr', e => callbacks.on_stderr(e.payload));

	const exit_code = await invoke<number>(
		'plugin:onelauncher|launch_cluster',
		{ uuid },
	);

	unlisten_launch();
	unlisten_stdout();
	unlisten_stderr();

	return exit_code;
}