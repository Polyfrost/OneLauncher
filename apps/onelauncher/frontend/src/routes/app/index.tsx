import DefaultBanner from '@/assets/images/default_banner.png';
import type { Model } from '@/bindings.gen';
import Button from '@/components/base/Button';
import useCommand from '@/hooks/useCommand';
import { bindings } from '@/main';
import { useIsFetching } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function RouteComponent() {
	const result = useCommand("getClusters", bindings.commands.getClusters)

	return (
		<div className="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />

			<div className="flex flex-row items-center justify-between">
				<div className="flex flex-row items-center gap-x-2">
					<ClusterCreate />
				</div>
			</div>

			<div className='h-96 overflow-y-auto'>
				<ClusterGroup clusters={result.data} />
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
	const result = useCommand("createCluster", () => bindings.commands.createCluster({
		icon_url: "asd",
		mc_loader: "vanilla",
		mc_version: "1.20.1",
		name: "Test Cluster",
		mc_loader_version: "0.13.5",
	}), {
		enabled: false,
		subscribed: false
	})

	const testThingy = () => {
		result.refetch()

		if (result.isError) {
			alert(result.error.message)
			return;
		}

		return;
	}

	return (
		<>
			<Button
				children="New Cluster"
				onClick={testThingy}
			/>
		</>
	)
}

interface ClusterGroupProps {
	clusters: Model[] | undefined;
}

function ClusterGroup(props: ClusterGroupProps) {
	const isFetching = useIsFetching();

	if (isFetching) {
		return (
			<div className='flex items-center justify-center h-fit'>
                <div className="w-8 h-8 border-4 border-brand rounded-full border-t-transparent animate-spin" />
			</div>
		)
	}

	return (
		<>
			<div className='grid grid-cols-4 gap-2'>
				{props.clusters?.map((data) => (
					<ClusterCard key={data.id} {...data} />
				))}
			</div>
		</>
	)
}

function ClusterCard(props: Model) {
	return (
		<>
			<div
				className="group relative h-[152px] flex flex-col rounded-xl border border-component-border/5 bg-component-bg active:bg-component-bg-pressed hover:bg-component-bg-hover"
			>
				<div className="relative flex-1 overflow-hidden rounded-t-xl">
					<div
						className="absolute h-full w-full transition-transform group-hover:!scale-110"
					>
						<img
							className="h-full w-full object-cover"
							src='https://github.com/emirsassan.png'
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
		</>
	);
}