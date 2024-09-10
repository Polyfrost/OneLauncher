import { useNavigate } from '@solidjs/router';
import {
	DotsVerticalIcon,
	PlayIcon,
	PlusIcon,
	RefreshCw01Icon,
	SearchMdIcon,
} from '@untitled-theme/icons-solid';
import { For, Show, onMount } from 'solid-js';
import type { Cluster } from '@onelauncher/client/bindings';
import BannerBackground from '../../assets/images/header.png';
import Button from '../components/base/Button';
import TextField from '../components/base/TextField';
import ClusterRoot from './cluster/ClusterRoot';
import { bridge } from '~imports';

import ClusterCover from '~ui/components/game/ClusterCover';
import { useClusterCreator } from '~ui/components/overlay/cluster/ClusterCreationModal';
import { useLaunchCluster, useRecentCluster } from '~ui/hooks/useCluster';
import useCommand from '~ui/hooks/useCommand';
import { formatAsDuration, upperFirst } from '~utils';

type GroupedClusters = Record<string, Cluster[]>;

function HomePage() {
	const [clusters, { refetch }] = useCommand(bridge.commands.getClustersGrouped);
	const controller = useClusterCreator();

	const containerIds = (list: GroupedClusters | undefined) => Object.keys(list || []);

	onMount(() => {
		bridge.events.clusterPayload.listen(({ payload }) => {
			if (payload.event === 'created' || payload.event === 'deleted')
				refetch();
		});
	});

	async function newCluster() {
		try {
			await controller.start();
		}
		catch (err) {
			console.error(err);
		}

		refetch();
	}

	return (
		<div class="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />

			<div class="flex flex-row items-center justify-between">
				<div>
					<TextField iconLeft={<SearchMdIcon />} placeholder="Search for clusters..." />
				</div>
				<div class="flex flex-row items-center gap-x-2">
					<Button
						buttonStyle="icon"
						children={<RefreshCw01Icon />}
						onClick={refetch}
					/>

					<Button
						buttonStyle="primary"
						children="New Cluster"
						iconLeft={<PlusIcon class="stroke-[2.2] !w-5" />}
						onClick={newCluster}
					/>
				</div>
			</div>

			<Show
				children={(
					<div class="flex flex-col gap-y-8 pb-8">
						<For each={Object.entries(clusters() ?? {})}>
							{([group, clusters]) => (
								<ClusterGroup clusters={clusters} title={group} />
							)}
						</For>
					</div>
				)}
				fallback={(
					<div class="max-h-64 flex flex-1 flex-col items-center justify-center gap-y-4">
						<span class="text-lg text-fg-secondary font-bold uppercase">No clusters were found.</span>
						<span class="text-xl font-bold">Create one now with the New Cluster button.</span>
					</div>
				)}
				when={containerIds(clusters()).length > 0}
			/>
		</div>
	);
}

export default HomePage;

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
	const launch = useLaunchCluster(() => cluster()?.uuid);
	const navigate = useNavigate();

	return (
		<div class="relative h-52 min-h-52 w-full overflow-hidden rounded-xl">
			<ClusterCover
				class="absolute h-52 w-full rounded-xl object-cover"
				cluster={cluster()}
				fallback={BannerBackground}
				linearBlur={{
					degrees: 270,
					blur: 30,
					class: 'after:right-1/3!',
				}}
			/>

			<div class="relative z-10 h-full flex flex-col items-start justify-between px-8 py-6 text-fg-primary">
				<div class="flex flex-col gap-y-2">
					<h1>{cluster()?.meta.name || 'Create a cluster'}</h1>
					<Show when={cluster() !== undefined}>
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
					</Show>
				</div>
				<div class="w-full flex flex-row items-end justify-between">
					<div class="flex flex-row items-center gap-x-4">
						<Show
							children={(
								<>
									<Button
										buttonStyle="primary"
										children={`Launch ${cluster()!.meta.mc_version}`}
										iconLeft={<PlayIcon />}
										onClick={launch}
									/>
									<Button
										buttonStyle="iconSecondary"
										children={<DotsVerticalIcon />}
										class="bg-op-10!"
										onClick={() => ClusterRoot.open(navigate, cluster()!.uuid)}
									/>
								</>
							)}
							when={cluster() !== undefined}
						/>
					</div>
					<div class="flex flex-row gap-x-2">
						{/* TODO: These tags */}
						{/* <Tag iconLeft={<OneConfigLogo />} />
						<Tag iconLeft={<CheckIcon />}>Verified</Tag> */}
					</div>
				</div>
			</div>
		</div>
	);
}

function ClusterCard(props: Cluster) {
	const navigate = useNavigate();

	function openClusterPage(_e: MouseEvent) {
		navigate(`/clusters/?id=${props.uuid}`);
	}

	const launch = useLaunchCluster(() => props.uuid);

	return (
		<>
			<div
				class="group relative h-[152px] flex flex-col border border-gray-05 rounded-xl bg-component-bg active:bg-component-bg-pressed hover:bg-component-bg-hover"
				onClick={e => openClusterPage(e)}
			>
				<div class="relative flex-1 overflow-hidden rounded-t-xl">
					<div
						class="absolute h-full w-full transition-transform group-hover:!scale-110"
						style={{ '-webkit-transform': 'translateZ(0)' }}
					>
						<ClusterCover
							class="h-full w-full object-cover"
							cluster={props}
						/>
					</div>
				</div>
				<div class="z-10 flex flex-row items-center justify-between gap-x-3 p-3">
					<div class="h-8 flex flex-col gap-1.5 overflow-hidden">
						<p class="h-4 text-ellipsis whitespace-nowrap font-medium">{props.meta.name}</p>
						<p class="h-4 text-xs">
							{upperFirst(props.meta.loader || 'Unknown')}
							{' '}
							{props.meta.mc_version}
							{/* {' '}
							{props.packages.mods && `• ${props.mods} mods`} */}
						</p>
					</div>
					<Button
						buttonStyle="iconSecondary"
						onClick={(e) => {
							e.preventDefault();
							e.stopImmediatePropagation();
							launch();
						}}
					>
						<PlayIcon class="h-4! w-4!" />
					</Button>
				</div>
			</div>
		</>
	);
}

interface ClusterGroupProps {
	title: string;
	clusters: Cluster[];
}

function ClusterGroup(props: ClusterGroupProps) {
	return (
		<div class="flex flex-col gap-y-4">
			<h4>{props.title}</h4>
			<div class="grid grid-cols-4 min-h-38 gap-4 2xl:grid-cols-6">
				<For each={props.clusters}>
					{item => <ClusterCard {...item} />}
				</For>
			</div>
		</div>
	);
}
