import type { ClusterModel } from '@/bindings.gen';
import type { ModInfo } from '.';
import { Link } from '@tanstack/react-router';

export function ModTag({ modData, cluster }: { modData: ModInfo; cluster: ClusterModel }) {
	return (
		<Link className="flex flex-row items-center justify-center px-4 rounded-full font-normal bg-component-bg border border-gray-100/5 scale-90" search={{ provider: 'Modrinth', packageId: modData.id ?? '', clusterId: cluster.id }} to="/app/cluster/browser/package">
			<p>{modData.managed ? 'Modrinth' : 'External'}</p>
		</Link>
	);
}
