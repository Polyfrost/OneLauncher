import { useSearchParams } from '@solidjs/router';
import { Show } from 'solid-js';
import { PlayIcon, Share07Icon } from '@untitled-theme/icons-solid';
import useCluster from '../../hooks/useCluster';
import ClusterCover from '../../components/game/ClusterCover';
import LoaderIcon from '../../components/game/LoaderIcon';
import Button from '../../components/base/Button';
import { launchCluster } from '../../../bridge/game';

function ClusterOverview() {
	const [params] = useSearchParams();
	const cluster = useCluster(params.id);
	if (cluster === null)
		throw new Error('Cluster doesn\'t exist');

	return (
		<div class="flex flex-col flex-1">
			<h1>Overview</h1>

			<Show when={!cluster.loading}>
				<Banner {...cluster()!} />
			</Show>
		</div>
	);
}

// eslint-disable-next-line solid/no-destructure
function Banner({ cluster, manifest }: Core.ClusterWithManifest) {
	return (
		<div class="flex flex-row bg-component-bg rounded-xl p-2.5 gap-x-2.5">
			<div class="w-48 rounded-lg overflow-hidden border border-gray-0.10">
				<ClusterCover cluster={cluster} />
			</div>

			<div class="flex flex-col flex-1 text-fg-primary">
				<div class="flex-1">
					<h2 class="text-2xl">{cluster.name}</h2>
					<span class="flex flex-row items-center gap-x-1">
						<LoaderIcon loader={cluster.client.type} class="w-5" />
						<span>{cluster.client.type}</span>
						<span>{manifest.manifest.id}</span>
					</span>
				</div>
				<span class="text-xs text-fg-secondary">
					Played for
					{' '}
					<b>3.9</b>
					{' '}
					hours
				</span>
			</div>

			<div class="flex flex-row items-end gap-x-2.5 *:h-8 *:w-8">
				<Button
					buttonStyle="secondary"
					iconLeft={<Share07Icon />}
				/>
				<Button
					buttonStyle="primary"
					iconLeft={<PlayIcon />}
					children="Launch"
					class="!w-auto"
					onClick={() => launchCluster(cluster.id)}
				/>
			</div>
		</div>
	);
}

export default ClusterOverview;
