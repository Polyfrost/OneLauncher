import { type JSX, splitProps } from 'solid-js';
import defaultCover from '~assets/images/default_instance_cover.jpg';
import type { Cluster } from '~bindings';

type ClusterCoverProps = JSX.HTMLAttributes<HTMLImageElement> & {
	cluster: Cluster;
};

function ClusterCover(props: ClusterCoverProps) {
	const [split, rest] = splitProps(props, ['cluster']);
	return (
		<img {...rest} src={split.cluster.meta.icon_url || defaultCover} />
	);
}

export default ClusterCover;
