import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { createFileRoute, Outlet } from '@tanstack/react-router';
import { EyeIcon, File06Icon, Globe04Icon, Image03Icon, PackagePlusIcon, Settings04Icon } from '@untitled-theme/icons-react';
import Sidebar from '../settings/route';

interface ClusterSearch {
	id: bigint;
}

export const Route = createFileRoute('/app/cluster')({
	component: RouteComponent,
	validateSearch: (search: Record<string, unknown>): ClusterSearch => {
		return {
			id: BigInt(search.id as string),
		};
	},
});

function RouteComponent() {
	const { id } = Route.useSearch();

	const cluster = useCommand('getClusterById', () => bindings.core.get_cluster(Number(id.toString()) as unknown as bigint));

	const isModded = cluster.data?.mc_loader !== 'vanilla';

	return (
		<div className="h-full flex flex-row overflow-hidden">
			<div className="flex-shrink-0 w-72 flex flex-col pt-8 pr-7 pb-8">
				<Sidebar
					base="/app/cluster"
					links={{
						'Cluster Settings': [
							// if someone has a better way to do this, please let me know
							[<EyeIcon key="overview" />, 'Overview', `/?id=${id}`],
							(isModded ? [<PackagePlusIcon key="mods" />, 'Mods', `/mods?id=${id}`] : undefined),
							[<Image03Icon key="screenshots" />, 'Screenshots', `/screenshots?id=${id}`],
							[<Globe04Icon key="worlds" />, 'Worlds', `/worlds?id=${id}`],
							[<File06Icon key="logs" />, 'Logs', `/logs?id=${id}`],
							[<Settings04Icon key="gamesettings" />, 'Game Settings', `/settings?id=${id}`],
						],
					}}
				/>
				{/* <Info /> */}
			</div>

			<div className="flex-1 min-w-0 h-full">
				<Outlet />
			</div>
		</div>
	);
}
