import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { Bundle, ModCardContext } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { useContext } from 'react';

function getBundleName(name: string): string {
	return (name.match(/\[(.*?)\]/)?.[1]) ?? 'Loading...';
}

interface ModListProps {
	bundles: Array<ModpackArchive>;
	cluster: ClusterModel;
	selectedTab?: string;
	onTabChange?: (value: string) => void;
}

export function ModList({ bundles, cluster, selectedTab, onTabChange }: ModListProps) {
	const ctx = useContext(ModCardContext);
	const useVerticalGridLayout = ctx?.useVerticalGridLayout ?? false;
	const { createSetting } = useSettings();
	const [useGridLayout, setUseGrid] = createSetting('mod_list_use_grid');

	const nonEmptyBundles = bundles.filter(b => b.manifest.files.length > 0);

	return (
		<Tabs defaultValue={selectedTab ?? getBundleName(nonEmptyBundles[0]?.manifest.name ?? bundles[0].manifest.name)} onTabChange={onTabChange}>
			<TabList className="justify-between px-4 py-3" height={false}>
				<div className="flex flex-row gap-6 px-2">
					{nonEmptyBundles.map(bundle => <Tab key={bundle.path} value={getBundleName(bundle.manifest.name)}>{getBundleName(bundle.manifest.name)}</Tab>)}
				</div>
				{!useVerticalGridLayout && (
					<Button onPress={() => setUseGrid(!useGridLayout)}>
						Toggle {useGridLayout ? 'Row' : 'Grid'}
					</Button>
				)}
			</TabList>

			<TabContent className={useVerticalGridLayout ? 'pt-0' : ''}>
				{nonEmptyBundles.map(bundleData => (
					<TabPanel className={useVerticalGridLayout ? 'max-w-192' : ''} key={getBundleName(bundleData.manifest.name)} value={getBundleName(bundleData.manifest.name)}>
						<Bundle cluster={cluster} files={bundleData.manifest.files} />
					</TabPanel>
				))}
			</TabContent>
		</Tabs>
	);
}
