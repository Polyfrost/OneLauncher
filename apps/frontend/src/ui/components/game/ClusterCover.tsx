import { convertFileSrc } from '@tauri-apps/api/core';
import { type JSX, type ParentProps, Show, splitProps } from 'solid-js';
import type { Cluster } from '@onelauncher/client/bindings';
import styles from './ClusterCover.module.scss';
import defaultCover from '~assets/images/default_instance_cover.jpg';

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
		<Show when={split.linearBlur !== undefined} fallback={<>{props.children}</>}>
			<div
				class={`${styles.linearBlur} ${split.linearBlur?.class || ''}`}
				style={{
					'--degrees': `${split.linearBlur?.degrees ?? 0}deg`,
					'--blur': `${split.linearBlur?.blur ?? 0}px`,
				}}
				children={props.children}
			/>
		</Show>
	);

	return (
		<Wrapper>
			<img
				{...rest}
				class={`${split.class || ''}`}
				src={image()}
				onError={(e) => {
					e.currentTarget.src = split.override ? convertFileSrc(split.override) : defaultCover;
					// @ts-expect-error -- JSX doesn't seem to use the same type
					split.onError?.(e);
				}}
			/>
		</Wrapper>
	);
}

export default ClusterCover;
