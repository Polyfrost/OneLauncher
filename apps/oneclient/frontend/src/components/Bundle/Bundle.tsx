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
		<OverlayScrollbarsComponent className='bg-page-elevated rounded-lg'>
			<div className={twMerge('grid gap-2 max-h-112 p-1', useGridLayout || useVerticalGridLayout ? 'grid-cols-3' : 'grid-cols-1')}>
				{bundleData.manifest.files.map(file => (
					<ModCard cluster={cluster} file={file} key={'Managed' in file.kind ? file.kind.Managed[0].id : file.kind.External.url} />
				))}
				<div className="h-1" />
			</div>
		</OverlayScrollbarsComponent>
	);
}
