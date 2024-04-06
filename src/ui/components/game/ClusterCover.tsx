import { type JSX, splitProps } from 'solid-js';
import defaultCover from '../../../assets/images/default_instance_cover.jpg';

type ClusterCoverProps = JSX.HTMLAttributes<HTMLImageElement> & {
	cluster: Core.Cluster;
};

function ClusterCover(props: ClusterCoverProps) {
	const [split, rest] = splitProps(props, ['cluster']);
	return (
		<img {...rest} src={split.cluster.cover || defaultCover} />
	);
}

export default ClusterCover;
