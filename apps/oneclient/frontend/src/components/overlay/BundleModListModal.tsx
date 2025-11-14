import type { ModpackFile } from '@/bindings.gen';
import type { ModCardContextApi, onClickOnMod } from '../Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { useMemo, useState } from 'react';
import { ModCardContext, ModList } from '../Bundle';
import { Overlay } from './Overlay';

export function BundleModListModal({ clusterId, name, setMods }: { clusterId: number; name: string; setMods: (value: React.SetStateAction<Array<ModpackFile>>) => void }) {
	const { data: cluster } = useCommandSuspense(['getClusterById'], () => bindings.core.getClusterById(clusterId));
	const { data: bundles } = useCommandSuspense(['getBundlesFor', clusterId], () => bindings.oneclient.getBundlesFor(clusterId));

	const onClickOnMod: onClickOnMod = (file, setShowOutline, setShowBlueBackground) => {
		setMods((prevMods) => {
			if (prevMods.includes(file))
				return prevMods.filter(mod => mod !== file);
			else
				return [file, ...prevMods];
		});
		setShowOutline(prev => !prev);
		setShowBlueBackground(prev => !prev);
	};

	const [tab, setSelectedTab] = useState<string>(name);

	const context = useMemo<ModCardContextApi>(() => ({
		onClickOnMod,
		useVerticalGridLayout: true,
	}), []);

	if (!cluster)
		return <></>;

	return (
		<Overlay.Dialog className="bg-page items-start">
			<Overlay.Title>
				Select Content for
				{' '}
				<span className="text-brand">{tab}</span>
			</Overlay.Title>
			<ModCardContext.Provider value={context}>
				<ModList
					bundles={bundles}
					cluster={cluster}
					onTabChange={setSelectedTab}
					selectedTab={name}
				/>
			</ModCardContext.Provider>
		</Overlay.Dialog>
	);
}
