import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { Bundle } from '.';

export function ModList({ bundles, cluster, showModDownload, onClickOnMod, defaultTab }: { bundles: Array<ModpackArchive>; cluster: ClusterModel; showModDownload?: boolean; onClickOnMod?: () => void; defaultTab?: string }) {
	return (
		<Tabs defaultValue={defaultTab ?? bundles[0].manifest.name}>
			<TabList className="gap-6">
				{bundles.map(bundle => <Tab key={bundle.manifest.name} value={bundle.manifest.name}>{(bundle.manifest.name.match(/\[(.*?)\]/)?.[1]) ?? 'LOADING'}</Tab>)}
			</TabList>

			<TabContent>
				{bundles.map(bundle => (
					<TabPanel key={bundle.manifest.name} value={bundle.manifest.name}>
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
