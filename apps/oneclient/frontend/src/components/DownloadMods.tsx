import type { ExternalPackage, ManagedVersionDependency, ModpackFile, ModpackFileKind, Provider } from '@/bindings.gen';
import { getModMetaDataName, Overlay } from '@/components';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useImperativeHandle, useState } from 'react';
import { DialogTrigger } from 'react-aria-components';

export interface DownloadModsRef {
	openDownloadDialog: (nextPath?: string) => void;
}

export interface BaseModData {
	name: string;
	clusterId: number;
	managed: boolean;
	bundleName: string | null;
	fileKind: ModpackFileKind;
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

export type DownloadModsParallelResult =
	| {
		ok: true;
		index: number;
		mod: ModData;
	}
	| {
		ok: false;
		index: number;
		mod: ModData;
		error: unknown;
	};

export function isManagedMod(mod: ModData): mod is ManagedModData {
	return mod.managed === true;
}

export interface ModWithBundle {
	file: ModpackFile;
	bundleName: string;
}

export function DownloadMods({ modsPerCluster, ref }: { modsPerCluster: Record<string, Array<ModWithBundle>>; ref: React.Ref<DownloadModsRef> }) {
	const navigate = useNavigate();
	const [isOpen, setOpen] = useState<boolean>(false);
	const [mods, setMods] = useState<ModDataArray>([]);
	const [nextPath, setNextPath] = useState<string>('/app');

	useEffect(() => {
		const modsList: ModDataArray = [];
		for (const [clusterId, modsWithBundle] of Object.entries(modsPerCluster))
			for (const { file: mod, bundleName } of modsWithBundle) {
				if ('External' in mod.kind)
					modsList.push({
						name: getModMetaDataName(mod),
						clusterId: Number(clusterId),
						managed: false,
						package: mod.kind.External,
						bundleName,
						fileKind: mod.kind,
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
						bundleName,
						fileKind: mod.kind,
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

export function buildModDataArray(modsPerCluster: Record<string, Array<ModWithBundle>>): ModDataArray {
	const modsList: ModDataArray = [];
	for (const [clusterId, modsWithBundle] of Object.entries(modsPerCluster))
		for (const { file: mod, bundleName } of modsWithBundle) {
			if ('External' in mod.kind)
				modsList.push({
					name: getModMetaDataName(mod),
					clusterId: Number(clusterId),
					managed: false,
					package: mod.kind.External,
					bundleName,
					fileKind: mod.kind,
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
					bundleName,
					fileKind: mod.kind,
				});
			}
		}
	return modsList;
}

export async function downloadModsParallel(
	items: ModDataArray,
	limit: number,
	fn: (mod: ModData, index: number) => Promise<void>,
): Promise<Array<DownloadModsParallelResult>> {
	if (items.length === 0)
		return [];

	let index = 0;
	const workerCount = Math.max(1, Math.min(limit, items.length));
	const results: Array<DownloadModsParallelResult> = [];

	const workers = Array.from({ length: workerCount }).fill(null).map(async () => {
		while (index < items.length) {
			const i = index;
			index += 1;

			if (i >= items.length)
				return;

			const mod = items[i];
			try {
				await fn(mod, i);
				results.push({
					ok: true,
					index: i,
					mod,
				});
			}
			catch (error) {
				results.push({
					ok: false,
					index: i,
					mod,
					error,
				});
			}
		}
	});

	await Promise.all(workers);

	results.sort((left, right) => left.index - right.index);
	return results;
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
					if (dependency.dependency_type === 'required') {
						const slug = dependency.project_id ?? '';
						const versions = await bindings.core.getPackageVersions(mod.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
						if (versions.items.length !== 0)
							await bindings.core.downloadPackage(mod.provider, slug, versions.items[0].version_id, cluster.id, null);
					}
				}

			if (mod.bundleName)
				await bindings.oneclient.downloadPackageFromBundle(
					mod.fileKind,
					mod.clusterId,
					mod.bundleName,
					true,
				);

			else
				await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
		}
		else {
			if (mod.bundleName)
				await bindings.oneclient.downloadPackageFromBundle(
					mod.fileKind,
					mod.clusterId,
					mod.bundleName,
					true,
				);

			else
				await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null);
		}
	});

	useEffect(() => {
		const downloadAll = async () => {
			const results = await downloadModsParallel(mods, 10, async (mod) => {
				setModName(mod.name);
				try {
					await download.mutateAsync(mod);
				}
				finally {
					setDownloadedMods(prev => prev + 1);
				}
			});

			const failed = results.filter(result => !result.ok);
			if (failed.length > 0)
				console.error('[DownloadMods] Some mods failed to download:', {
					failed: failed.length,
					total: mods.length,
					entries: failed.map(result => ({
						index: result.index,
						name: result.mod.name,
						error: result.error,
					})),
				});
		};

		void downloadAll();
	// eslint-disable-next-line react-hooks/exhaustive-deps -- download is not stable, adding it would cause infinite loops
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
