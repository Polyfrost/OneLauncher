import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { useSettings } from '@/hooks/useSettings';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { Bundle, useModCardContext } from '.';

function getBundleName(name: string): string {
	return (name.match(/\[(.*?)\]/)?.[1]) ?? 'LOADING';
}

interface ModListProps {
	bundles: Array<ModpackArchive>;
	cluster: ClusterModel;
	selectedTab?: string;
	onTabChange?: (value: string) => void;
}

export function ModList({ bundles, cluster, selectedTab, onTabChange }: ModListProps) {
	const { useVerticalGridLayout } = useModCardContext();
	const { createSetting } = useSettings();
	const [useGridLayout, setUseGrid] = createSetting('mod_list_use_grid');

	return (
		<Tabs defaultValue={selectedTab ?? getBundleName(bundles[0].manifest.name)} onTabChange={onTabChange}>
			<TabList className="justify-between px-4">
				<div className="flex flex-row gap-6 px-2">
					{bundles.map(bundle => <Tab key={getBundleName(bundle.manifest.name)} value={getBundleName(bundle.manifest.name)}>{getBundleName(bundle.manifest.name)}</Tab>)}
				</div>
				{!useVerticalGridLayout && (
					<Button onPress={() => setUseGrid(!useGridLayout)}>
						Toggle {useGridLayout ? 'Row' : 'Grid'}
					</Button>
				)}
			</TabList>

			<TabContent className="pt-0">
				{bundles.map(bundleData => (
					<TabPanel className={useVerticalGridLayout ? 'max-w-192' : ''} key={getBundleName(bundleData.manifest.name)} value={getBundleName(bundleData.manifest.name)}>
						<Bundle bundleData={bundleData} cluster={cluster} />
					</TabPanel>
				))}
			</TabContent>
		</Tabs>
	);
}
