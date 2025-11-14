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
			<div className={twMerge('grid gap-2', useGridLayout ? 'grid-cols-3' : 'grid-cols-1', useGridLayout && useVerticalGridLayout ? 'max-h-128' : 'max-h-112')}>
				{bundleData.manifest.files.map((file, index) => (
					<ModCard cluster={cluster} file={file} key={index} />
				))}
			</div>
		</OverlayScrollbarsComponent>
	);
}
