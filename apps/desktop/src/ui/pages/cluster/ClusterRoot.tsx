import { Route, useSearchParams } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import { EyeIcon, File06Icon, Globe04Icon, Image03Icon, Settings04Icon } from '@untitled-theme/icons-solid';
import Sidebar from '../../components/Sidebar';
import AnimatedRoutes from '../../components/AnimatedRoutes';
import ErrorBoundary from '../../components/ErrorBoundary';
import ClusterOverview from './cluster/ClusterOverview';
import ClusterLogs from './cluster/ClusterLogs';
import ClusterMods from './cluster/ClusterMods';
import ClusterScreenshots from './cluster/ClusterScreenshots';
import ClusterSettings from './cluster/ClusterSettings';
import useClusterContext, { ClusterProvider } from '~ui/hooks/useCluster';

function ClusterRoutes() {
	return (
		<>
			<Route path="/" component={ClusterOverview} />
			<Route path="/logs" component={ClusterLogs} />
			<Route path="/mods" component={ClusterMods} />
			<Route path="/screenshots" component={ClusterScreenshots} />
			<Route path="/settings" component={ClusterSettings} />
		</>
	);
}

function ClusterRoot(props: ParentProps) {
	const [searchParams] = useSearchParams();

	return (
		<ClusterProvider uuid={searchParams.id}>
			<div class="flex flex-row flex-1 h-full gap-x-7">
				<div class="mt-8">
					<ClusterSidebar />
				</div>

				<div class="flex flex-col w-full h-full">
					<AnimatedRoutes>
						<ErrorBoundary>
							{props.children}
						</ErrorBoundary>
					</AnimatedRoutes>
				</div>
			</div>
		</ClusterProvider>
	);
}

function ClusterSidebar() {
	const cluster = useClusterContext();

	return (
		<Sidebar
			base="/clusters"
			state={{ id: cluster.uuid }}
			links={{
				Cluster: [
					[<EyeIcon />, 'Overview', '/'],
					// TODO: Better way of checking mods
					// (cluster!.meta.loader === 'vanilla' ? [<PackagePlusIcon />, 'Mods', '/mods'] : undefined),
					[<Image03Icon />, 'Screenshots', '/screenshots'],
					[<Globe04Icon />, 'Worlds', '/worlds'],
					[<File06Icon />, 'Logs', '/logs'],
					[<Settings04Icon />, 'Game Settings', '/settings'],
				],
			}}
		/>
	);
}

ClusterRoot.Routes = ClusterRoutes;

export default ClusterRoot;
