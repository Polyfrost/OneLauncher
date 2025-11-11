import type { ClusterModel, ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { ModInfo } from '.';
import { useSettings } from '@/hooks/useSettings';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { twMerge } from 'tailwind-merge';
import { ModCard } from '.';

export function Bundle({ bundleData, cluster, showModDownload, onClickOnMod, outline }: { bundleData: ModpackArchive; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>, setShowBlueBackground: React.Dispatch<React.SetStateAction<boolean>>) => void; outline?: boolean }) {
	const { setting } = useSettings();
	const grid = setting('mod_list_use_grid');

	return (
		<OverlayScrollbarsComponent>
			<div className={twMerge('grid gap-2 max-h-112', grid ? 'grid-cols-3' : 'grid-cols-1')}>
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
