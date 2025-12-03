import type { ExternalPackage, ManagedVersionDependency, ModpackFile, Provider } from '@/bindings.gen';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useImperativeHandle, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';
import { getModMetaDataName } from './Bundle';
import { Overlay } from './overlay';
import { useSettings } from '@/hooks/useSettings';

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
	dependencies: Array<ManagedVersionDependency>;
}

export interface ExternalModData extends BaseModData {
	package: ExternalPackage;
}

export type ModData = ManagedModData | ExternalModData;
export type ModDataArray = Array<ModData>;

export function isManagedMod(mod: ModData): mod is ManagedModData {
	return mod.managed === true;
}

export function DownloadMods({ modsPerCluster, ref }: { modsPerCluster: Record<string, Array<ModpackFile>>; ref: React.Ref<DownloadModsRef> }) {
	const navigate = useNavigate();
	const [isOpen, setOpen] = useState<boolean>(false);
	const [mods, setMods] = useState<ModDataArray>([]);
	const [nextPath, setNextPath] = useState<string>('/app');

	useEffect(() => {
		const modsList: ModDataArray = [];
		for (const [clusterId, mods] of Object.entries(modsPerCluster))
			for (const mod of mods) {
				if ('External' in mod.kind)
					modsList.push({
						name: getModMetaDataName(mod),
						clusterId: Number(clusterId),
						managed: false,
						package: mod.kind.External,
					});

				if ('Managed' in mod.kind) {
					const [pkg, version] = mod.kind.Managed;
					modsList.push({
						name: getModMetaDataName(mod),
						clusterId: Number(clusterId),
						managed: true,
						provider: pkg.provider,
						id: pkg.id,
						versionId: version.version_id,
						dependencies: version.dependencies,
					});
				}
			}
		setMods(modsList);
	}, [modsPerCluster]);

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

function downloadModsParallel(items: ModDataArray, limit: number, fn: (mod: ModData, index: number) => Promise<void>) {
	let index = 0;
	const workers = Array.from({ length: limit }).fill(null).map(async () => {
		while (index < items.length) {
			const i = index++;
			await fn(items[i], i);
		}
	});
	return Promise.all(workers);
}

function DownloadingMods({ mods, setOpen, nextPath }: { mods: ModDataArray; setOpen: React.Dispatch<React.SetStateAction<boolean>>; nextPath: string }) {
	const navigate = useNavigate();
	const [downloadedMods, setDownloadedMods] = useState(0);
	const [modName, setModName] = useState<string | null>(null);
	const download = useCommandMut(async (mod: ModData) => {
		if (isManagedMod(mod)) {
			if (mod.dependencies.length > 0)
				for (const dependency of mod.dependencies) {
					const cluster = await bindings.core.getClusterById(mod.clusterId);
					if (!cluster)
						continue;
					const slug = dependency.project_id ?? '';
					const versions = await bindings.core.getPackageVersions(mod.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
					if (versions.items.length !== 0)
						await bindings.core.downloadPackage(mod.provider, slug, versions.items[0].version_id, cluster.id, null);
				}
			await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
		}
		else { await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null); }
	});

	const { setting } = useSettings();
	let useSlowDownloading = setting('slow_mod_bulk_downloading');


	useEffect(() => {
		const downloadAll = async () => {
			if (useSlowDownloading)
				for (const mod of mods) {
					setModName(mod.name);
					try {
						await download.mutateAsync(mod);
					}
					finally {
						setDownloadedMods(prev => prev + 1);
					}
				} else
				await downloadModsParallel(mods, 25, async (mod) => {
					setModName(mod.name);
					try {
						await download.mutateAsync(mod);
					}
					finally {
						setDownloadedMods(prev => prev + 1);
					}
				});

		};

		downloadAll();
	}, [mods]);

	useEffect(() => {
		if (downloadedMods >= mods.length) {
			setOpen(false);
			navigate({ to: nextPath });
		}
	}, [downloadedMods, mods, navigate, nextPath, setOpen]);

	return (
		<Overlay.Dialog isDismissable={false}>
			<Overlay.Title>Downloading Mods</Overlay.Title>

			<div className="w-full flex flex-col items-center gap-2">
				<p>Downloaded {downloadedMods} / {mods.length}</p>
				<div className="w-1/2 h-4 bg-component-bg-disabled rounded-full outline-2 outline-ghost-overlay">
					<div
						className="h-full bg-brand rounded-full transition-all duration-300"
						style={{ width: mods.length > 0 ? `${(downloadedMods / mods.length) * 100}%` : '0%' }}
					>
					</div>
				</div>
				{modName !== null ? <p>Downloading {modName}</p> : <></>}
			</div>

		</Overlay.Dialog>
	);
}
