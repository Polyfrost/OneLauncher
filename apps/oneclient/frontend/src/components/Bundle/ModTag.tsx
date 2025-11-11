import type { ClusterModel } from '@/bindings.gen';
import type { ModInfo } from '.';
import { useSettings } from '@/hooks/useSettings';
import { Link } from '@tanstack/react-router';
import { twMerge } from 'tailwind-merge';

export function ModTag({ modData, cluster }: { modData: ModInfo; cluster: ClusterModel }) {
	const { setting } = useSettings();
	const grid = setting('mod_list_use_grid');

	return (
		<Link className={twMerge('flex flex-row items-center justify-center px-4 rounded-full font-normal bg-component-bg border border-gray-100/5 h-8 scale-90', grid ? '-my-1' : '')} search={{ provider: 'Modrinth', packageId: modData.id ?? '', clusterId: cluster.id }} to="/app/cluster/browser/package">
			<p>{modData.managed ? 'Modrinth' : 'External'}</p>
		</Link>
	);
}
