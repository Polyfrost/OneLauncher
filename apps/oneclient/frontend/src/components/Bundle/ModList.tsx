import type { ClusterModel, ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { ModInfo } from '.';
import { Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { Bundle } from '.';

function getBundleName(name: string): string {
	return (name.match(/\[(.*?)\]/)?.[1]) ?? 'LOADING';
}

export function ModList({ bundles, cluster, showModDownload, onClickOnMod, defaultTab, outline }: { bundles: Array<ModpackArchive>; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>) => void; defaultTab?: string; outline?: boolean }) {
	return (
		<Tabs defaultValue={defaultTab ?? getBundleName(bundles[0].manifest.name)}>
			<TabList className="gap-6">
				{bundles.map(bundle => <Tab key={getBundleName(bundle.manifest.name)} value={getBundleName(bundle.manifest.name)}>{getBundleName(bundle.manifest.name)}</Tab>)}
			</TabList>

			<TabContent>
				{bundles.map(bundle => (
					<TabPanel key={getBundleName(bundle.manifest.name)} value={getBundleName(bundle.manifest.name)}>
						<Bundle
							bundleData={bundle}
							cluster={cluster}
							onClickOnMod={onClickOnMod}
							outline={outline}
							showModDownload={showModDownload}
						/>
					</TabPanel>
				))}
			</TabContent>
		</Tabs>
	);
}
