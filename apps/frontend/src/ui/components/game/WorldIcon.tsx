import { convertFileSrc } from '@tauri-apps/api/core';
import { join } from 'pathe';
import { type JSX, splitProps } from 'solid-js';
import useSettings from '~ui/hooks/useSettings';
import DefaultWorld from '~assets/images/default_world.png?url';

export type WorldIconProps = JSX.HTMLAttributes<HTMLImageElement> & {
	world_name: string;
	cluster_name: string;
};

function WorldIcon(props: WorldIconProps) {
	const [split, rest] = splitProps(props, ['cluster_name', 'world_name', 'class']);
	const { settings } = useSettings();
	const path = () => convertFileSrc(join(settings().config_dir || '', 'clusters', props.cluster_name, 'saves', props.world_name, 'icon.png'));

	return (
		<img
			{...rest}
			class={`w-16 h-16 rounded-md ${split.class || ''}`}
			src={path()}
			onError={(e) => {
				e.currentTarget.src = DefaultWorld;
			}}
			alt={`${props.world_name} icon`}
		/>
	);
}

export default WorldIcon;
