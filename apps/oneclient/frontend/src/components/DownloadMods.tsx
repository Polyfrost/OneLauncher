import type { ExternalPackage, ManagedVersionDependency, ModpackArchive, ModpackFile, Provider } from '@/bindings.gen';
import { getModMetaDataName, Overlay } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useImperativeHandle, useRef, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';

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

export function DownloadMods({ modsPerCluster, bundlesPerCluster, ref }: { modsPerCluster: Record<string, Array<ModpackFile>>; bundlesPerCluster?: Record<string, Array<ModpackArchive>>; ref: React.Ref<DownloadModsRef> }) {
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
				<DownloadingMods bundlesPerCluster={bundlesPerCluster} mods={mods} nextPath={nextPath} setOpen={setOpen} />
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

function DownloadingMods({ mods, setOpen, nextPath, bundlesPerCluster }: { mods: ModDataArray; setOpen: React.Dispatch<React.SetStateAction<boolean>>; nextPath: string; bundlesPerCluster?: Record<string, Array<ModpackArchive>> }) {
	const navigate = useNavigate();
	const [downloadedMods, setDownloadedMods] = useState(0);
	const [modName, setModName] = useState<string | null>(null);
	const [allDone, setAllDone] = useState(false);
	const hasStarted = useRef(false);
	const download = useCommandMut(async (mod: ModData) => {
		if (isManagedMod(mod)) {
			if (mod.dependencies.length > 0)
				for (const dependency of mod.dependencies) {
					const cluster = await bindings.core.getClusterById(mod.clusterId);
					if (!cluster)
						continue;
					if (dependency.dependency_type === 'required') {
						const slug = dependency.project_id ?? '';
						const versions = await bindings.core.getPackageVersions(mod.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
						if (versions.items.length !== 0)
							await bindings.core.downloadPackage(mod.provider, slug, versions.items[0].version_id, cluster.id, null);
					}
				}
			await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
		}
		else { await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null); }
	});

	const { setting } = useSettings();
	let useParallelModDownloading = setting('parallel_mod_downloading');

	useEffect(() => {
		if (hasStarted.current)
			return;
		hasStarted.current = true;

		const downloadAll = async () => {
			try {
				if (useParallelModDownloading)
					await downloadModsParallel(mods, 10, async (mod) => {
						setModName(mod.name);
						try {
							await download.mutateAsync(mod);
						}
						catch (e) {
							console.warn(`Failed to download mod ${mod.name}:`, e);
						}
						finally {
							setDownloadedMods(prev => prev + 1);
						}
					}); else
					for (const mod of mods) {
						setModName(mod.name);
						try {
							await download.mutateAsync(mod);
						}
						catch (e) {
							console.warn(`Failed to download mod ${mod.name}:`, e);
						}
						finally {
							setDownloadedMods(prev => prev + 1);
						}
					}

				// Extract overrides from enabled bundles after all mods are downloaded
				if (bundlesPerCluster) {
					setModName('Extracting overrides...');
					for (const [clusterId, bundles] of Object.entries(bundlesPerCluster)) {
						for (const bundle of bundles) {
							if (bundle.manifest.enabled) {
								try {
									await bindings.oneclient.extractBundleOverrides(bundle.path, Number(clusterId));
								}
								catch (e) {
									console.error(`Failed to extract overrides for bundle ${bundle.manifest.name}:`, e);
								}
							}
						}
					}
				}
			}
			catch (e) {
				console.error('Error during mod download:', e);
			}
			finally {
				setAllDone(true);
			}
		};

		downloadAll();
	}, [mods]);

	useEffect(() => {
		if (allDone) {
			setOpen(false);
			navigate({ to: nextPath });
		}
	}, [allDone, navigate, nextPath, setOpen]);

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
