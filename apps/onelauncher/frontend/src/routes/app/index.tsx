import type { Model } from '@/bindings.gen';
import DefaultBanner from '@/assets/images/default_banner.png';
import HeaderImage from '@/assets/images/default_banner.png';
import DefaultInstancePhoto from '@/assets/images/default_instance_cover.jpg';
import Button from '@/components/base/Button';
import { TextField } from '@/components/base/TextField';
import Modal from '@/components/overlay/Modal';
import useCommand from '@/hooks/useCommand';
import { bindings } from '@/main';
import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { Server01Icon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

/*
Please note this route has a very big issue related to scrolling
and i am very angry rn so i will not be fixing it rn
*/
function RouteComponent() {
	const result = useCommand('getClusters', bindings.core.get_clusters);
	const test = useCommand('ajgiagwo', bindings.onelauncher.return_error, {
		enabled: true,
		retry: false,
	});

	return (
		<div className="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />

			<div className="flex flex-row items-center justify-between">
				<div className="flex flex-row items-center gap-x-2">
					<ClusterCreate />
				</div>
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
	// const cluster = useRecentCluster();
	// const launch = useLaunchCluster(() => cluster()?.uuid);
	const navigate = useNavigate();

	return (
		<div className="relative h-52 min-h-52 w-full overflow-hidden rounded-xl border border-component-border">
			<div className="absolute h-52 min-h-52 w-full">
				<img
					alt="Default Banner"
					className="top-0 left-0 h-full w-full object-cover"
					src={DefaultBanner}
				/>
			</div>

			<div className="relative z-10 h-full flex flex-col items-start justify-between px-8 py-6">
				<div className="theme-OneLauncher-Dark flex flex-col gap-y-2 text-fg-primary">
					<h1 className="text-2xl font-medium text-shadow-black text-shadow-2xs">Create a cluster</h1>
					{/* <Show when={cluster() !== undefined}>
						<p>
							You've played
							{' '}
							<strong>
								{cluster()!.meta.mc_version}
								{' '}
								{upperFirst(cluster()!.meta.loader || 'Unknown')}
							</strong>
							{' '}
							for
							{' '}
							<strong>{formatAsDuration((cluster()!.meta.overall_played || 0))}</strong>
							.
						</p>
					</Show> */}
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

function ClusterCreate() {
	const result = useCommand('createCluster', () => bindings.commands.createCluster({
		icon_url: 'asd',
		mc_loader: 'vanilla',
		mc_version: '1.20.1',
		name: 'Test Cluster',
		mc_loader_version: '0.13.5',
	}), {
		enabled: false,
		subscribed: false,
	});

	const testThingy = () => {
		result.refetch();

		if (result.isError)
			alert(result.error.message);
	};

	return (
		<>
			<Modal.Trigger>
				<Button
					children="New Cluster"
				// onClick={testThingy}
				/>

				<Modal>
					<div className="min-w-sm flex flex-col rounded-lg bg-page text-center">
						<div className="theme-OneLauncher-Dark relative h-25 flex">
							<div className="absolute left-0 top-0 h-full w-full">
								<img alt="Header Image" className="h-full w-full rounded-t-lg" src={HeaderImage} />
							</div>
							<div
								className="absolute left-0 top-0 h-full flex flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10"
							>
								<Server01Icon className="h-8 w-8 text-fg-primary" />
								<div className="flex flex-col items-start justify-center">
									<h1 className="h-10 text-fg-primary -mt-2">New Cluster</h1>
									{/* <span className="text-fg-primary">asd</span> */}
								</div>
							</div>
						</div>
						<div className="flex flex-col border border-white/5 rounded-b-lg">
							<div className="p-3">
								<TextField className="px-2" placeholder="Epik cluster name" />
							</div>

							<div className="flex flex-row justify-end gap-x-2 p-3 pt-0">
								<Button
									children="Create"
									color="primary"
									onClick={testThingy}
								/>
							</div>
						</div>
					</div>
				</Modal>
			</Modal.Trigger>
		</>
	);
}

interface ClusterGroupProps {
	clusters: Array<Model> | undefined;
	isFetching?: boolean;
}

function ClusterGroup(props: ClusterGroupProps) {
	if (props.isFetching)
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
					{props.clusters?.map(data => (
						<ClusterCard key={data.id} {...data} />
					))}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

function ClusterCard(props: Model) {
	return (
		<Link
			search={{
				id: props.id,
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
							src={DefaultInstancePhoto}
						/>
					</div>
				</div>
				<div className="z-10 flex flex-row items-center justify-between gap-x-3 p-3">
					<div className="h-full flex flex-col gap-1.5 overflow-hidden">
						<p className="h-4 text-ellipsis whitespace-nowrap font-medium">{props.name}</p>
						<p className="h-4 text-xs">
							{props.mc_loader}
							{' '}
							{props.mc_version}
							{/* {' '}
						{props.packages.mods && `• ${props.mods} mods`} */}
						</p>
					</div>

					{/* <LaunchButton cluster={props} iconOnly /> */}
				</div>
			</div>
		</Link>
	);
}
