import DefaultWorld from '@/assets/images/default_world.png';
import { convertFileSrc } from '@tauri-apps/api/core';
import { dataDir, join } from '@tauri-apps/api/path';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export interface WorldIconProps {
	world_name: string;
	cluster_name: string;
	className?: string;
}

export default function WorldIcon(props: WorldIconProps) {
	const { world_name, cluster_name, className } = props;
	const [imagePath, setImagePath] = useState<string>('');

	useEffect(() => {
		async function loadImagePath() {
			const iconPath = await join(await dataDir(), 'OneLauncher', 'clusters', cluster_name, 'saves', world_name, 'icon.png');
			setImagePath(convertFileSrc(iconPath));
		}

		loadImagePath().catch(console.error);
	}, [cluster_name, world_name]);

	return (
		<img
			{...props}
			alt={`${world_name} icon`}
			className={twMerge('rounded-md size-16', className)}
			onError={(e) => {
				e.currentTarget.src = DefaultWorld;
			}}
			src={imagePath}
		/>
	);
}
