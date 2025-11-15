import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useState } from 'react';
import { ModList, useModCardContext } from '../Bundle';
import { Overlay } from './Overlay';

export function BundleModListModal({ clusterId, name }: { clusterId: number; name: string }) {
	const { data: cluster } = useCommandSuspense(['getClusterById'], () => bindings.core.getClusterById(clusterId));
	const { data: bundles } = useCommandSuspense(['getBundlesFor', clusterId], () => bindings.oneclient.getBundlesFor(clusterId));
	const [tab, setSelectedTab] = useState<string>(name);
	const { mods } = useModCardContext();

	if (!cluster)
		return <></>;

	return (
		<Overlay.Dialog className="bg-page items-start">
			<Overlay.Title>
				Select Content for
				{' '}
				<span className="text-brand">{tab}</span>
			</Overlay.Title>
			<ModList
				bundles={bundles}
				cluster={cluster}
				onTabChange={setSelectedTab}
				selectedTab={name}
			/>
			<div className="w-full flex flex-row gap-8 items-center justify-end bg-page-elevated p-4 rounded-2xl pt-3">
				<p className="font-normal text-fg-secondary">Selected {mods?.length ?? 0} mods</p>
				<Button color="primary" size="large" slot="close">Confirm</Button>
			</div>
		</Overlay.Dialog>
	);
}
