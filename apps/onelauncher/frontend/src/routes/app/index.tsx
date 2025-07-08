import type { ClusterModel } from '@/bindings.gen';
import DefaultBanner from '@/assets/images/default_banner.png';
import DefaultInstancePhoto from '@/assets/images/default_instance_cover.jpg';
import { NewClusterCreate } from '@/components/launcher/cluster/ClusterCreation';
import useRecentCluster from '@/hooks/useCluster';
import { bindings } from '@/main';
import { upperFirst } from '@/utils';
import { useCommand } from '@onelauncher/common';
import { Button, Show } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';
import { convertFileSrc } from '@tauri-apps/api/core';
import { PlayIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useEffect } from 'react';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

/*
Please note this route has a very big issue related to scrolling
and i am very angry rn so i will not be fixing it rn

hey future sassan here i guess the issue is solved idk

hey future sassan here i guess the issue is solved idk
*/
function RouteComponent() {
	const result = useCommand('getClusters', bindings.core.getClusters);

	return (
		<div className="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />

			<div className="flex flex-row items-center justify-between">
				<NewClusterCreate />
			</div>

			<div className="flex flex-col">
				<ClusterGroup clusters={result.data} isFetching={result.isFetching} />
			</div>
		</div>
	);
}

function Banner() {
	/**
	 * If there are any clusters, display the most recent cluster name + some statistics as the "description".
	 * The background would prioritise
	 * any screenshots taken from the cluster, if there are none, it would display the cluster cover if it has been set.
	 * The button would launch the cluster.
	 *
	 * If there are no clusters, display a generic background with the button action creating a new cluster.
	 */
	const cluster = useRecentCluster();

	// const launch = useLaunchCluster(() => cluster()?.uuid);
	// const navigate = useNavigate();

	return (
		<div className="relative h-52 min-h-52 w-full overflow-hidden rounded-xl border border-component-border">
			<div className="absolute h-52 min-h-52 w-full">
				<img
					alt="Default Banner"
					className="top-0 left-0 h-full rounded-xl w-full object-cover"
					onError={(e) => {
						(e.target as HTMLImageElement).src = DefaultBanner;
					}}
					src={cluster?.icon_url || DefaultBanner}
				/>
			</div>

			<div className="relative z-10 h-full flex flex-col items-start justify-between px-8 py-6">
				<div className="theme-OneLauncher-Dark flex flex-col gap-y-2 text-fg-primary">
					<h1 className="text-2xl font-medium text-shadow-black text-shadow-2xs">Create a cluster</h1>
					<Show when={cluster !== undefined}>
						<p>
							You've played
							{' '}
							<strong>
								{cluster?.name}
								{' '}
								{upperFirst(cluster?.mc_loader || 'Unknown')}
							</strong>
							{' '}
							for
							{' '}
							<strong>{cluster?.overall_played || 0}</strong>
							.
						</p>
					</Show>
				</div>
				<div className="w-full flex flex-row items-end justify-between">
					<div className="flex flex-row items-center gap-x-4">
						{/* <Show when={cluster() !== undefined}>
							<Button
								buttonStyle="primary"
								children={`Launch ${cluster()!.meta.mc_version}`}
								iconLeft={<PlayIcon />}
								onClick={launch}
							/>
							<Button
								buttonStyle="iconSecondary"
								children={<Settings01Icon />}
								className="theme-OneLauncher-Dark bg-op-10!"
								onClick={() => ClusterRoot.open(navigate, cluster()!.uuid)}
							/>
						</Show> */}
					</div>
					<div className="flex flex-row gap-x-2">
						{/* TODO: These tags */}
						{/* <Tag iconLeft={<OneConfigLogo />} />
						<Tag iconLeft={<CheckIcon />}>Verified</Tag> */}
					</div>
				</div>
			</div>
		</div>
	);
}

interface ClusterGroupProps {
	clusters: Array<ClusterModel> | undefined;
	isFetching?: boolean;
}

function ClusterGroup({
	isFetching,
	clusters,
}: ClusterGroupProps) {
	if (isFetching)
		return (
			<div className="flex items-center justify-center h-full">
				<div className="w-8 h-8 border-4 border-brand rounded-full border-t-transparent animate-spin" />
			</div>
		);

	return (
		<div className="h-full w-full">
			<OverlayScrollbarsComponent
				className="h-full w-full"
			>
				<div className="grid grid-cols-4 gap-4 max-h-96 2xl:grid-cols-6 pb-4">
					{clusters?.map(data => (
						<ClusterCard key={data.id} {...data} />
					))}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

function ClusterCard({
	id,
	name,
	mc_loader,
	mc_version,
	icon_url,
}: ClusterModel) {
	const launch = useCommand('launchCluster', () => bindings.core.launchCluster(id, null), {
		enabled: false,
		subscribed: false,
	});

	const handleLaunch = () => {
		launch.refetch();

		if (launch.error)
			console.error(launch.error.message);
	};

	const image = () => {
		const url = icon_url;

		if (!url)
			return DefaultInstancePhoto;

		return convertFileSrc(url);
	};

	// eslint-disable-next-line no-console -- ok
	console.log(image());

	return (
		<>
			<Link
				search={{
					id,
				}} to="/app/cluster"
			>
				<div
					className="group relative h-[152px] flex flex-col rounded-xl border border-component-border/5 bg-component-bg active:bg-component-bg-pressed hover:bg-component-bg-hover"
				>
					<div className="relative flex-1 overflow-hidden rounded-t-xl">
						<div
							className="absolute h-full w-full transition-transform group-hover:!scale-110"
						>
							<img
								className="h-full w-full object-cover"
								onError={(e) => {
									(e.target as HTMLImageElement).src = DefaultInstancePhoto;
								}}
								src={image()}
							/>
						</div>
					</div>
					<div className="z-10 flex flex-row items-center justify-between gap-x-3 p-3">
						<div className="h-full flex flex-col gap-1.5 overflow-hidden">
							<p className="h-4 text-ellipsis whitespace-nowrap font-medium">
								{name}
							</p>
							<p className="h-4 text-xs">
								{mc_loader}
								{' '}
								{mc_version}
							</p>
						</div>

						{/* <LaunchButton cluster={props} iconOnly /> */}
						<Button onClick={handleLaunch} size="icon"><PlayIcon /></Button>
					</div>
				</div>
			</Link>
		</>
	);
}
