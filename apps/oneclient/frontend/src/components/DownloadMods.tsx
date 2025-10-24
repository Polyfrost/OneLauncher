import type { Provider } from '@/bindings.gen';
import type { BundleData } from '@/routes/onboarding/preferences/versions';
import { Button } from '@onelauncher/common/components';
import { useEffect, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { DownloadingMods, Overlay } from './overlay';

interface ModData {
	name: string;
	provider: Provider;
	id: string;
	versionId: string;
	clusterId: number;
}

export function DownloadMods({ bundlesData }: { bundlesData: Record<string, BundleData> }) {
	const [isOpen, setOpen] = useState<boolean>(false);
	const [mods, setMods] = useState<Array<ModData>>([]);

	useEffect(() => {
		const modsList: Array<ModData> = [];
		for (const bundle of Object.values(bundlesData))
			for (const mod of bundle.modsInfo[0]) {
				if (!('Managed' in mod.kind))
					continue;
				const [pkg, version] = mod.kind.Managed;
				modsList.push({
					name: pkg.name,
					provider: pkg.provider,
					id: pkg.id,
					versionId: version.version_id,
					clusterId: bundle.clusterId,
				});
			}
		setMods(modsList);
	}, [bundlesData]);

	return (
		<DialogTrigger>
			<Button className="mb-4" isDisabled={mods.length === 0} onPress={() => setOpen(prev => !prev)}>Download Mods</Button>

			<Overlay isDismissable={false} isOpen={isOpen}>
				<DownloadingMods mods={mods} setOpen={setOpen} />
			</Overlay>
		</DialogTrigger>
	);
}
