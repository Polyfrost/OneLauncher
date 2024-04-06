import type { Resource } from 'solid-js';
import { createResource } from 'solid-js';
import { getCluster } from '../../bridge/game';

function useCluster(uuid: string | undefined | null): Resource<Core.ClusterWithManifest> | null {
	if (typeof uuid !== 'string' || uuid.length === 0)
		return null;

	const [resource] = createResource(uuid, getCluster);
	return resource;
}

export default useCluster;
