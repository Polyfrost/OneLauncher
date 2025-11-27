import type { ExternalPackage, Provider } from '@/bindings.gen';
import type { BundleData } from '@/routes/onboarding/preferences/version';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useImperativeHandle, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { getModMetaDataName } from './Bundle';
import { DownloadingMods, Overlay } from './overlay';

export interface DownloadModsRef {
	openDownloadDialog: (nextPath?: string) => void;
}

export interface BaseModData {
	name: string;
	clusterId: number;
	managed: boolean;
}

export interface ManagedModData extends BaseModData {
	provider: Provider;
	id: string;
	versionId: string;
}

export interface ExternalModData extends BaseModData {
	package: ExternalPackage;
}

export function isManagedMod(mod: ManagedModData | ExternalModData): mod is ManagedModData {
	return mod.managed === true;
}

export function DownloadMods({ bundlesData, ref }: { bundlesData: Record<string, BundleData>; ref: React.Ref<DownloadModsRef> }) {
	const navigate = useNavigate();
	const [isOpen, setOpen] = useState<boolean>(false);
	const [mods, setMods] = useState<Array<ManagedModData | ExternalModData>>([]);
	const [nextPath, setNextPath] = useState<string>('/app');

	useEffect(() => {
		const modsList: Array<ManagedModData | ExternalModData> = [];
		for (const bundle of Object.values(bundlesData))
			for (const mod of bundle.modsInfo[0]) {
				if ('External' in mod.kind)
					modsList.push({
						name: getModMetaDataName(mod),
						clusterId: bundle.clusterId,
						managed: false,
						package: mod.kind.External,
					});

				if ('Managed' in mod.kind) {
					const [pkg, version] = mod.kind.Managed;
					modsList.push({
						name: getModMetaDataName(mod),
						clusterId: bundle.clusterId,
						managed: true,
						provider: pkg.provider,
						id: pkg.id,
						versionId: version.version_id,
					});
				}
			}
		setMods(modsList);
	}, [bundlesData]);

	useImperativeHandle(ref, () => {
		return {
			openDownloadDialog(nextPath?: string) {
				if (mods.length !== 0) {
					setOpen(true);
					setNextPath(nextPath ?? '/app');
				}
				else {
					navigate({ to: nextPath ?? '/app' });
				}
			},
		};
	}, [mods.length, navigate]);

	return (
		<DialogTrigger>
			<Button className="mb-4" isDisabled={mods.length === 0} onPress={() => setOpen(prev => !prev)}>Download Mods</Button>

			<Overlay isDismissable={false} isOpen={isOpen}>
				<DownloadingMods mods={mods} nextPath={nextPath} setOpen={setOpen} />
			</Overlay>
		</DialogTrigger>
	);
}
