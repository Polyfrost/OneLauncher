import type { ClusterModel, ModpackArchive, ModpackFile } from '@/bindings.gen';
import type { ModInfo } from '.';
import { useSettings } from '@/hooks/useSettings';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { Bundle } from '.';

function getBundleName(name: string): string {
	return (name.match(/\[(.*?)\]/)?.[1]) ?? 'LOADING';
}

export function ModList({ bundles, cluster, showModDownload, onClickOnMod, selectedTab, onTabChange }: { bundles: Array<ModpackArchive>; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>, setShowBlueBackground: React.Dispatch<React.SetStateAction<boolean>>) => void; selectedTab?: string; onTabChange?: (value: string) => void }) {
	const { createSetting } = useSettings();
	const [useGrid, setUseGrid] = createSetting('mod_list_use_grid');

	return (
		<Tabs defaultValue={selectedTab ?? getBundleName(bundles[0].manifest.name)} onTabChange={onTabChange}>
			<TabList className="justify-between">
				<div className="flex flex-row gap-6">
					{bundles.map(bundle => <Tab key={getBundleName(bundle.manifest.name)} value={getBundleName(bundle.manifest.name)}>{getBundleName(bundle.manifest.name)}</Tab>)}
				</div>
				<Button onPress={() => setUseGrid(!useGrid)}>
					Toggle {useGrid ? 'Row' : 'Grid'} - {selectedTab}
				</Button>
			</TabList>

			<TabContent>
				{bundles.map(bundle => (
					<TabPanel key={getBundleName(bundle.manifest.name)} value={getBundleName(bundle.manifest.name)}>
						<Bundle
							bundleData={bundle}
							cluster={cluster}
							onClickOnMod={onClickOnMod}
							showModDownload={showModDownload}
						/>
					</TabPanel>
				))}
			</TabContent>
		</Tabs>
	);
}
