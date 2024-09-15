import type { Cluster } from '@onelauncher/client/bindings';
import { convertFileSrc } from '@tauri-apps/api/core';
import defaultCover from '~assets/images/default_instance_cover.jpg';
import { type JSX, type ParentProps, Show, splitProps } from 'solid-js';
import styles from './ClusterCover.module.scss';

type ClusterCoverProps = JSX.HTMLAttributes<HTMLImageElement> & {
	cluster: Cluster | undefined;
	override?: string;
	fallback?: string;
	linearBlur?: {
		degrees?: number;
		blur?: number;
		class?: string;
	};
};

function ClusterCover(props: ClusterCoverProps) {
	const [split, rest] = splitProps(props, ['cluster', 'override', 'onError', 'fallback', 'linearBlur', 'class']);

	const image = () => {
		const url = split.override || split.cluster?.meta.icon_url || split.cluster?.meta.icon;

		if (url === undefined || url === null)
			return split.fallback || defaultCover;

		if (url.startsWith('/'))
			return convertFileSrc(url);

		return url;
	};

	const Wrapper = (props: ParentProps) => (
		<Show fallback={<>{props.children}</>} when={split.linearBlur !== undefined}>
			<div
				children={props.children}
				class={`${styles.linearBlur} ${split.linearBlur?.class || ''}`}
				style={{
					'--degrees': `${split.linearBlur?.degrees ?? 0}deg`,
					'--blur': `${split.linearBlur?.blur ?? 0}px`,
				}}
			/>
		</Show>
	);

	return (
		<Wrapper>
			<img
				{...rest}
				class={`${split.class || ''}`}
				onError={(e) => {
					e.currentTarget.src = split.override ? convertFileSrc(split.override) : defaultCover;
					if (typeof split.onError === 'function')
						split.onError(e);
				}}
				src={image()}
			/>
		</Wrapper>
	);
}

export default ClusterCover;
