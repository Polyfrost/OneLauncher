import type { ModpackFile } from '@/bindings.gen';
import type { ModInfo } from '../Bundle';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { ModList } from '../Bundle';
import { Overlay } from './Overlay';

export function BundleModListModal({ clusterId, name, setMods }: { clusterId: number; name: string; setMods: (value: React.SetStateAction<Array<ModpackFile>>) => void }) {
	const { data: cluster } = useCommandSuspense(['getClusterById'], () => bindings.core.getClusterById(clusterId));
	const { data: bundles } = useCommandSuspense(['getBundlesFor', clusterId], () => bindings.oneclient.getBundlesFor(clusterId));

	const onClickOnMod = (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>) => {
		setMods((prevMods) => {
			if (prevMods.includes(file))
				return prevMods.filter(mod => mod !== file);
			else
				return [file, ...prevMods];
		});
		setShowOutline(prev => !prev);
	};

	if (!cluster)
		return <></>;

	return (
		<Overlay.Dialog className="bg-page">
			<ModList
				bundles={bundles}
				cluster={cluster}
				defaultTab={name}
				onClickOnMod={onClickOnMod}
			/>
		</Overlay.Dialog>
	);
}
