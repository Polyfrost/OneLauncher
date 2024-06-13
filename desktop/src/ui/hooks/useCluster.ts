import type { Resource } from 'solid-js';
import useCommand from './useCommand';
import type { Cluster } from '~bindings';
import { bridge } from '~index';

function useCluster(uuid: string | undefined | null): Resource<Cluster> | null {
	if (typeof uuid !== 'string' || uuid.length === 0)
		return null;

	const [resource] = useCommand(bridge.commands.getCluster, uuid);
	return resource;
}

export default useCluster;
