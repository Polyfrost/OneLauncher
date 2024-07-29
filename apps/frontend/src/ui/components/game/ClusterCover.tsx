import { convertFileSrc } from '@tauri-apps/api/core';
import { type JSX, splitProps } from 'solid-js';
import defaultCover from '~assets/images/default_instance_cover.jpg';
import type { Cluster } from '~bindings';

type ClusterCoverProps = JSX.HTMLAttributes<HTMLImageElement> & {
	cluster: Cluster | undefined;
	override?: string;
};

function ClusterCover(props: ClusterCoverProps) {
	const [split, rest] = splitProps(props, ['cluster', 'override', 'onError']);

	const image = () => {
		const url = split.override || split.cluster?.meta.icon_url || split.cluster?.meta.icon;

		if (url === undefined || url === null)
			return defaultCover;

		if (url.startsWith('/'))
			return convertFileSrc(url);

		return url;
	};

	return (
		<img
			{...rest}
			src={image()}
			onError={(e) => {
				e.currentTarget.src = split.override ? convertFileSrc(split.override) : defaultCover;
				if (split.onError)
					// @ts-expect-error -- jsx stuff
					split.onError!(e);
			}}
		/>
	);
}

export default ClusterCover;
