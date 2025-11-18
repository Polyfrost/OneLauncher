import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { useSettings } from '@/hooks/useSettings';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { twMerge } from 'tailwind-merge';
import { ModCard, useModCardContext } from '.';

interface BundleProps {
	bundleData: ModpackArchive;
	cluster: ClusterModel;
}

export function Bundle({ bundleData, cluster }: BundleProps) {
	const { useVerticalGridLayout } = useModCardContext();
	const { setting } = useSettings();
	const useGridLayout = setting('mod_list_use_grid');

	return (
		<OverlayScrollbarsComponent>
			<div className={twMerge('grid gap-2 max-h-112 p-1 bg-page-elevated rounded-lg', useGridLayout || useVerticalGridLayout ? 'grid-cols-3' : 'grid-cols-1')}>
				{bundleData.manifest.files.map((file, index) => (
					<ModCard cluster={cluster} file={file} key={index} />
				))}
				<div className="h-1" />
			</div>
		</OverlayScrollbarsComponent>
	);
}
