import { invoke } from '@tauri-apps/api/core';

export async function getInstance(uuid: string): Promise<Core.Instance> {
	return await invoke<Core.Instance>('plugin:onelauncher|get_instance', {
		uuid,
	});
}

export async function getManifest(uuid: string): Promise<Core.Manifest> {
	return await invoke<Core.Manifest>('plugin:onelauncher|get_manifest', {
		uuid,
	});
}

export async function getInstancesWithManifests(): Promise<Core.InstanceWithManifest[]> {
	const instances = await getInstances();

	return Promise.all(instances.map(async instance => ({
		instance,
		manifest: await getManifest(instance.id),
	})));
}

export async function getGroupedInstances(): Promise<Map<string, Core.InstanceWithManifest[]>> {
	const instances = await getInstancesWithManifests();
	const map = new Map<string, Core.InstanceWithManifest[]>();

	instances.forEach((instance, index) => {
		const groupName = instance.instance.group || 'Unnamed';
		const value = { ...instance, index };

		if (map.has(groupName))
			map.get(groupName)!.push(value);
		else
			map.set(groupName, [value]);
	});

	return map;
}

export async function getInstances(): Promise<Core.Instance[]> {
	return await invoke<Core.Instance[]>('plugin:onelauncher|get_instances');
}

export async function createInstance(instance: Omit<Core.Instance, 'id' | 'createdAt'> & { version: string }): Promise<void> {
	return await invoke(
		'plugin:onelauncher|create_instance',
		{
			name: instance.name,
			version: instance.version,
			client: instance.client,
			// cover: instance.cover,
			// group: instance.group,
		},
		{
			headers: {
				Accept: 'text/plain',
			},
		},
	);
}

export async function refreshClientManager(): Promise<void> {
	return await invoke('plugin:onelauncher|refresh_client_manager');
}
