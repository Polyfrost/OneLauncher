import DefaultBanner from '@/assets/images/default_banner.png';
import { createFileRoute, useNavigate } from '@tanstack/react-router';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />
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
