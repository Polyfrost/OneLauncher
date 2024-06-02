import type { Resource } from 'solid-js';
import { createResource } from 'solid-js';

function getCluster(uuid: string): Core.ClusterWithManifest {
	throw new Error('get cluster');
}

function useCluster(uuid: string | undefined | null): Resource<Core.ClusterWithManifest> | null {
	if (typeof uuid !== 'string' || uuid.length === 0)
		return null;

	const [resource] = createResource(uuid, getCluster);
	return resource;
}

export default useCluster;
