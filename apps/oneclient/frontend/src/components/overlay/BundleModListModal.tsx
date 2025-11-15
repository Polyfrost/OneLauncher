import type { ModpackFile } from '@/bindings.gen';
import type { ModCardContextApi, onClickOnMod } from '../Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useMemo, useState } from 'react';
import { ModCardContext, ModList } from '../Bundle';
import { Overlay } from './Overlay';

export function BundleModListModal({ clusterId, name, mods, setMods }: { clusterId: number; name: string; mods: Array<ModpackFile>; setMods: (value: React.SetStateAction<Array<ModpackFile>>) => void }) {
	const { data: cluster } = useCommandSuspense(['getClusterById'], () => bindings.core.getClusterById(clusterId));
	const { data: bundles } = useCommandSuspense(['getBundlesFor', clusterId], () => bindings.oneclient.getBundlesFor(clusterId));

	const onClickOnMod: onClickOnMod = (file, setSelected) => {
		setMods((prevMods) => {
			if (prevMods.includes(file))
				return prevMods.filter(mod => mod !== file);
			else
				return [file, ...prevMods];
		});
		setSelected(prev => !prev);
	};

	const [tab, setSelectedTab] = useState<string>(name);

	const context = useMemo<ModCardContextApi>(() => ({
		onClickOnMod,
		useVerticalGridLayout: true,
		mods,
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
			<div className="w-full flex flex-row gap-8 items-center justify-end bg-page-elevated p-4 rounded-2xl pt-3">
				<p className="font-normal text-fg-secondary">Selected {mods.length} mods</p>
				<Button color="primary" size="large" slot="close">Confirm</Button>
			</div>
		</Overlay.Dialog>
	);
}
