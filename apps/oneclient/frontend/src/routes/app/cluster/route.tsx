import { LoaderSuspense, Tab, TabList } from '@/components';
import { LaunchButton } from '@/components/LaunchButton';
import { SheetPage } from '@/components/SheetPage';
import { useIsRunning } from '@/hooks/useClusters';
import { bindings } from '@/main';
import { prettifyLoader } from '@/utils/loaders';
import { getVersionInfoOrDefault } from '@/utils/versionMap';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Outlet, redirect } from '@tanstack/react-router';
import { FolderIcon } from '@untitled-theme/icons-react';
import { useMotionValueEvent, useScroll } from 'motion/react';
import { useRef, useState } from 'react';

export interface ClusterRouteSearchParams {
	clusterId: number;
}

export const Route = createFileRoute('/app/cluster')({
	component: RouteComponent,
	validateSearch: (search): ClusterRouteSearchParams => {
		return {
			clusterId: Number(search.clusterId),
		};
	},
	async beforeLoad({ context, search }) {
		if (!search.clusterId)
			throw redirect({ to: '/app/clusters', from: '/app/cluster' });

		const query = context.queryClient.ensureQueryData({
			queryKey: ['getClusterById', search.clusterId],
			queryFn: () => bindings.core.getClusterById(search.clusterId),
		});

		const cluster = await query;
		if (!cluster)
			throw redirect({ to: '/app/clusters', from: '/app/cluster' });

		return {
			cluster,
		};
	},
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const tabListRef = useRef<HTMLDivElement>(null);
	const scrollContainerRef = useRef<HTMLElement>(null);

	const isRunning = useIsRunning(cluster.id);

	const { scrollYProgress } = useScroll({
		axis: 'y',
		container: scrollContainerRef,
		target: tabListRef,
		layoutEffect: false,
		offset: ['end end', 'start start'],
	});

	const [floating, setFloating] = useState(false);
	useMotionValueEvent(scrollYProgress, 'change', (v) => {
		setFloating(v >= 0.9);
	});

	const search = Route.useSearch();

	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
			scrollContainerRef={scrollContainerRef}
		>
			<TabList floating={floating} ref={tabListRef}>
				<Tab from={Route.id} search={search} to="/app/cluster/overview">Overview</Tab>
				<Tab from={Route.id} search={search} to="/app/cluster/logs">Logs</Tab>
				<Tab from={Route.id} search={{ ...search, packageType: 'mod' }} to="/app/cluster/browser">Browser</Tab>
				{isRunning && <span className="flex-1"></span>}
				{isRunning && <Tab from={Route.id} search={search} to="/app/cluster/process">Process</Tab>}
			</TabList>

			<div className="relative pb-8">
				<LoaderSuspense spinner={{ size: 'large' }}>
					<Outlet />
				</LoaderSuspense>
			</div>
		</SheetPage>
	);
}

function HeaderLarge() {
	const { cluster } = Route.useRouteContext();
	const versionInfo = getVersionInfoOrDefault(cluster.mc_version);

	const openFolder = () => bindings.folders.openCluster(cluster.folder_name);

	return (
		<div className="flex flex-row items-end gap-16">
			<div className="flex-1 flex flex-col gap-2">
				<div className="flex flex-row gap-2 flex-wrap">
					{versionInfo.tags.map(tag => (
						<span className="text-sm font-medium text-fg-secondary bg-component-bg/40 px-2 py-1 rounded" key={tag}>
							{tag}
						</span>
					))}
				</div>
				<h1 className="text-3xl font-semibold">{prettifyLoader(cluster.mc_loader)} {cluster.mc_version}</h1>
				<p className="text-md font-medium text-fg-secondary">{versionInfo.longDescription}</p>
			</div>

			<div className="flex flex-row gap-2">
				<Button
					color="secondary"
					onPress={openFolder}
					size="iconLarge"
				>
					<FolderIcon />
				</Button>

				<LaunchButton
					clusterId={cluster.id}
					size="large"
				/>
			</div>
		</div>
	);
}

function HeaderSmall() {
	const { cluster } = Route.useRouteContext();

	const openFolder = () => bindings.folders.openCluster(cluster.folder_name);

	return (
		<div className="flex flex-row justify-between items-center h-full">
			<h1 className="text-2lg h-full font-medium">{prettifyLoader(cluster.mc_loader)} {cluster.mc_version}</h1>

			<div className="flex flex-row gap-2">
				<Button
					color="secondary"
					onPress={openFolder}
					size="icon"
				>
					<FolderIcon />
				</Button>

				<LaunchButton
					clusterId={cluster.id}
					color="primary"
				/>
			</div>
		</div>
	);
}
