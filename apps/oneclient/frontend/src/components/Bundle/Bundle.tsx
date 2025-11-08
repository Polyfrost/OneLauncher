import type { ClusterModel, ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { ModInfo } from '.';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { ModCard } from '.';

export function Bundle({ bundleData, cluster, showModDownload, onClickOnMod, outline }: { bundleData: ModpackArchive; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>) => void; outline?: boolean }) {
	return (
		<OverlayScrollbarsComponent>
			<div className="grid gap-2 grid-cols-1 h-112">
				{bundleData.manifest.files.map((file, index) => (
					<ModCard
						cluster={cluster}
						file={file}
						key={index}
						onClick={onClickOnMod}
						outline={outline}
						showDownload={showModDownload}
					/>
				))}
			</div>
		</OverlayScrollbarsComponent>
	);
}
