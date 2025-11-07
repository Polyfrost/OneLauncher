import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { ModCard } from '.';

export function Bundle({ bundleData, cluster, showModDownload, onClickOnMod }: { bundleData: ModpackArchive; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: () => void }) {
	return (
		<OverlayScrollbarsComponent>
			<div className="grid gap-2 grid-cols-1 h-112">
				{bundleData.manifest.files.map((file, index) => <ModCard cluster={cluster} file={file} key={index} onClick={onClickOnMod} showDownload={showModDownload} />)}
			</div>
		</OverlayScrollbarsComponent>
	);
}
