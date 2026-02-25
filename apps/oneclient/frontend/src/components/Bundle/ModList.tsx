import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { Bundle, ModCardContext, getModMetaDataName } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs, TextField } from '@onelauncher/common/components';
import { SearchMdIcon } from '@untitled-theme/icons-react';
import Fuse from 'fuse.js';
import { useContext, useMemo, useState } from 'react';

function getBundleName(name: string): string {
	return (name.match(/\[(.*?)\]/)?.[1]) ?? 'Loading...';
}

interface ModListProps {
	bundles: Array<ModpackArchive>;
	cluster: ClusterModel;
	selectedTab?: string;
	onTabChange?: (value: string) => void;
	toggleBundlePaths?: Set<string>;
}

export function ModList({ bundles, cluster, selectedTab, onTabChange, toggleBundlePaths }: ModListProps) {
	const ctx = useContext(ModCardContext);
	const useVerticalGridLayout = ctx?.useVerticalGridLayout ?? false;
	const { createSetting } = useSettings();
	const [useGridLayout, setUseGrid] = createSetting('mod_list_use_grid');
	const [searchQuery, setSearchQuery] = useState('');

	const nonEmptyBundles = bundles
		.map(bundle => ({
			...bundle,
			manifest: {
				...bundle.manifest,
				files: bundle.manifest.files.filter(file => !file.hidden),
			},
		}))
		.filter(b => b.manifest.files.length > 0);

	const isSearching = searchQuery.trim().length > 0;

	const searchResults = useMemo(() => {
		if (!isSearching)
			return [];
		const allFiles = nonEmptyBundles.flatMap(b => b.manifest.files);
		const fuse = new Fuse(allFiles, {
			keys: [{ name: 'name', getFn: file => getModMetaDataName(file) }],
			threshold: 0.4,
			distance: 100,
			minMatchCharLength: 1,
		});
		return fuse.search(searchQuery.trim()).map(r => r.item);
	}, [isSearching, searchQuery, nonEmptyBundles]);

	return (
		<Tabs defaultValue={selectedTab ?? getBundleName(nonEmptyBundles[0]?.manifest.name ?? bundles[0].manifest.name)} onTabChange={onTabChange}>
			<TabList className="justify-between px-4 py-3" height={false}>
				{!isSearching && (
					<div className="flex flex-row gap-6 px-2">
						{nonEmptyBundles.map(bundle => <Tab key={bundle.path} value={getBundleName(bundle.manifest.name)}>{getBundleName(bundle.manifest.name)}</Tab>)}
					</div>
				)}
				{isSearching && <div />}
				<div className="flex flex-row items-center gap-2">
					<TextField
						className="w-48"
						iconLeft={<SearchMdIcon className="scale-75" />}
						onChange={e => setSearchQuery(e.target.value)}
						placeholder="Search..."
						value={searchQuery}
					/>
					{!useVerticalGridLayout && (
						<Button onPress={() => setUseGrid(!useGridLayout)}>
							Toggle {useGridLayout ? 'Row' : 'Grid'}
						</Button>
					)}
				</div>
			</TabList>

			{isSearching
				? (
					<div className={useVerticalGridLayout ? 'pt-0' : 'p-2'}>
						<Bundle cluster={cluster} files={searchResults} />
					</div>
				)
				: (
					<TabContent className={useVerticalGridLayout ? 'pt-0' : ''}>
						{nonEmptyBundles.map((bundleData) => {
							const isToggle = toggleBundlePaths?.has(bundleData.path) ?? false;
							const content = <Bundle cluster={cluster} files={bundleData.manifest.files} />;

							return (
								<TabPanel className={useVerticalGridLayout ? 'max-w-192' : ''} key={getBundleName(bundleData.manifest.name)} value={getBundleName(bundleData.manifest.name)}>
									{isToggle
										? (
												<ModCardContext.Provider value={{ ...ctx, useToggleMode: true }}>
													{content}
												</ModCardContext.Provider>
											)
										: content}
								</TabPanel>
							);
						})}
					</TabContent>
				)}
		</Tabs>
	);
}
