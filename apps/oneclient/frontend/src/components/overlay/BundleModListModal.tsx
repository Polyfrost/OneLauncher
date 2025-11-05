import type { ClusterModel, ModpackArchive } from '@/bindings.gen';
import { bindings } from '@/main';
import { useState } from 'react';
import { Bundle } from '../BundleModsList';
import { Overlay } from './Overlay';

export function BundleModListModal({ bundleData, cluster, name }: { bundleData: ModpackArchive; cluster: ClusterModel | null; name: string }) {
	const [mods, setMods] = useState<Array<string>>([]);
	const updateMods = () => {
		(async () => {
			if (!cluster)
				return;
			setMods(await bindings.core.getMods(cluster.id));
		})();
	};

	if (!cluster)
		return <></>;
	return (
		<Overlay.Dialog>
			<Overlay.Title>Mod List for {name}</Overlay.Title>
			<Bundle
				bundleData={bundleData}
				cluster={cluster}
				mods={mods}
				updateMods={updateMods}
			/>
		</Overlay.Dialog>
	);
}
