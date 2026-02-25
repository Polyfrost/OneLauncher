import type { ClusterModel, ModpackFile, ModpackFileKind, PackageModel } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { ModTag } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export interface ModInfo {
	name: string;
	description: string | null;
	author: string | null;
	iconURL: string | null;
	url: string | null;
	managed: boolean;
	packageSlug: string | null;
}

async function getModAuthor(kind: ModpackFileKind, useVerticalGridLayout?: boolean): Promise<string | null> {
	if ('External' in kind)
		return null;
	if (!('Managed' in kind))
		return null;

	const authors = await bindings.core.getUsersFromAuthor(kind.Managed[0].provider, kind.Managed[0].author);
	return parseAuthors(authors.map(author => author.username), useVerticalGridLayout);
}

function parseAuthors(usernames: Array<string>, useVerticalGridLayout?: boolean): string | null {
	if (usernames.length === 0)
		return null;

	if (useVerticalGridLayout) {
		let visibleCount = usernames.length;
		while (visibleCount >= 1) {
			const hiddenUsers = usernames.length - visibleCount;
			let string = usernames.slice(0, visibleCount).join(', ');
			if (hiddenUsers > 0)
				string += ` and ${hiddenUsers} more`;
			if (string.length <= 21)
				return string;
			visibleCount--;
		}
		return `${usernames[0]} and ${usernames.length - 1} more`;
	}
	return usernames.join(', ');
}

export function getModMetaDataName(file: ModpackFile): string {
	if ('External' in file.kind)
		return file.overrides?.name ?? file.kind.External.name.replaceAll('.jar', '');
	if ('Managed' in file.kind)
		return file.overrides?.name ?? file.kind.Managed[0].name;
	else return 'UNKNOWN';
}

function getFileKey(file: ModpackFile): string {
	if ('Managed' in file.kind)
		return `managed:${file.kind.Managed[0].id}`;
	if ('External' in file.kind)
		return `external:${file.kind.External.sha1 ?? file.kind.External.url ?? file.kind.External.name}`;
	return 'unknown';
}

const modMetadataCache = new Map<string, Promise<ModInfo>>();

function fetchModMetaData(file: ModpackFile, useVerticalGridLayout?: boolean): Promise<ModInfo> {
	const key = getFileKey(file);
	const existing = modMetadataCache.get(key);
	if (existing)
		return existing;

	const promise = getModMetaData(file, useVerticalGridLayout).catch((err) => {
		modMetadataCache.delete(key);
		throw err;
	});
	modMetadataCache.set(key, promise);
	return promise;
}

async function getModMetaData(file: ModpackFile, useVerticalGridLayout?: boolean): Promise<ModInfo> {
	if ('External' in file.kind)
		return {
			name: getModMetaDataName(file),
			description: file.overrides?.description ?? null,
			author: parseAuthors(file.overrides?.authors ?? [], useVerticalGridLayout),
			iconURL: file.overrides?.icon ?? null,
			url: null,
			managed: false,
			packageSlug: null,
		};

	return {
		name: getModMetaDataName(file),
		description: file.overrides?.description ?? file.kind.Managed[0].short_desc,
		author: (file.overrides?.authors ?? []).length > 1 ? parseAuthors(file.overrides?.authors ?? []) : await getModAuthor(file.kind, useVerticalGridLayout),
		iconURL: file.overrides?.icon ?? file.kind.Managed[0].icon_url,
		url: `https://modrinth.com/project/${file.kind.Managed[0].slug}`,
		managed: true,
		packageSlug: file.kind.Managed[0].id,
	};
}

interface ModCardProps {
	file: ModpackFile;
	cluster: ClusterModel;
}

export type onClickOnMod = (file: ModpackFile) => void;
export interface ModCardContextApi {
	enableClickToDownload?: boolean;
	onClickOnMod?: onClickOnMod;
	useVerticalGridLayout?: boolean;
	mods?: Array<ModpackFile>;
	installedPackages?: Array<PackageModel>;
	useToggleMode?: boolean;
}

const DEFAULT_MOD_CARD_CONTEXT: ModCardContextApi = {};

export const ModCardContext = createContext<ModCardContextApi | null>(null);
export function useModCardContext() {
	return useContext(ModCardContext) ?? DEFAULT_MOD_CARD_CONTEXT;
}

export function ModCard({ file, cluster }: ModCardProps) {
	const { enableClickToDownload, onClickOnMod, useVerticalGridLayout, mods, installedPackages, useToggleMode } = useModCardContext();
	const queryClient = useQueryClient();

	const [modMetadata, setModMetadata] = useState<ModInfo>({ name: getModMetaDataName(file), description: null, author: null, iconURL: null, url: null, managed: false, packageSlug: null });
	useEffect(() => {
		let cancelled = false;
		fetchModMetaData(file, useVerticalGridLayout).then((meta) => {
			if (!cancelled)
				setModMetadata(meta);
		}).catch(() => {});
		return () => { cancelled = true; };
	}, [file, useVerticalGridLayout]);

	const kind = file.kind;
		// Directory-based packages (folders dropped into resourcepacks/shaderpacks/datapacks)
		// cannot be toggled via .disabled rename — skip toggle mode for them.
		const isDirectory = useMemo(() =>
			'External' in kind && !kind.External.name.includes('.'),
		[kind]);
		const effectiveToggleMode = useToggleMode && !isDirectory;
	const isInstalled = useMemo(() => {
		if (installedPackages)
			if ('Managed' in kind) {
				const [pkg, _] = kind.Managed;
				return installedPackages.some(p => p.package_id === pkg.id && p.provider === pkg.provider);
			}
			else {
				return installedPackages.some(p => p.hash === kind.External.sha1);
			}

		return mods?.includes(file) ?? false;
	}, [installedPackages, kind, mods, file]);

	// In toggle mode, "selected" = enabled (file_name doesn't end with .disabled)
	const isFileEnabled = useMemo(() => {
			if (!effectiveToggleMode)
				return true;
			if ('Managed' in kind) {
				const primary = kind.Managed[1].files.find(f => f.primary) ?? kind.Managed[1].files[0];
				return !primary?.file_name.endsWith('.disabled');
			}
			return !kind.External.name.endsWith('.disabled');
		}, [effectiveToggleMode, kind]);
		const [isSelected, setSelected] = useState(effectiveToggleMode ? isFileEnabled : isInstalled);
		useEffect(() => {
			setSelected(effectiveToggleMode ? isFileEnabled : isInstalled);
		}, [isInstalled, effectiveToggleMode, isFileEnabled]);

	const download = useCommandMut(async () => {
		if ('Managed' in kind) {
			const [pkg, version] = kind.Managed;
			if (version.dependencies.length > 0)
				for (const dependency of version.dependencies)
					if (dependency.dependency_type === 'required') {
						const slug = dependency.project_id ?? '';
						const versions = await bindings.core.getPackageVersions(pkg.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
						await bindings.core.downloadPackage(pkg.provider, slug, versions.items[0].version_id, cluster.id, null);
					}

			await bindings.core.downloadPackage(pkg.provider, pkg.id, version.version_id, cluster.id, null);
		}
		else {
			await bindings.core.downloadExternalPackage(kind.External, cluster.id, null, null);
		}
	});

	const remove = useCommandMut(async () => {
		let hash: string | undefined;
		if ('Managed' in kind) {
			const [_, version] = kind.Managed;
			const primary = version.files.find(f => f.primary) ?? version.files[0];
			hash = primary.sha1;
		}
		else {
			hash = kind.External.sha1;
		}

		if (hash)
			await bindings.core.removePackage(cluster.id, hash);
	});

	const toggle = useCommandMut(async () => {
		let hash: string | undefined;
		if ('Managed' in kind) {
			const [_, version] = kind.Managed;
			const primary = version.files.find(f => f.primary) ?? version.files[0];
			hash = primary.sha1;
		}
		else {
			hash = kind.External.sha1;
		}

		if (hash)
			await bindings.core.togglePackage(cluster.id, hash);
	});

	const handleOnClick = () => {
			// Folder-based packages (resource packs, shader packs, data packs as directories)
			// cannot be toggled or removed — treat them as entirely non-interactive.
			if (isDirectory)
				return;
			if (effectiveToggleMode)
			(async () => {
				await toggle.mutateAsync();
				await queryClient.invalidateQueries({ queryKey: ['getLinkedPackages', cluster.id] });
			})();

		else if (enableClickToDownload)
			(async () => {
				if (isInstalled)
					await remove.mutateAsync();

				else
					await download.mutateAsync();

				await queryClient.invalidateQueries({ queryKey: ['getLinkedPackages', cluster.id] });
			})();

		else if (onClickOnMod)
			onClickOnMod(file);
	};

	const { setting } = useSettings();
	let useGridLayout = setting('mod_list_use_grid');
	if (useVerticalGridLayout)
		useGridLayout = true;

	return (
		<div className={twMerge('rounded-lg m-1 break-inside-avoid flex bg-component-bg border border-gray-100/5 p-2 gap-2 outline-2 outline-component-bg', useVerticalGridLayout && useGridLayout ? '' : 'justify-between', useGridLayout ? 'flex-col' : 'flex-row', isSelected ? 'outline-brand bg-brand/20' : '')} onClick={handleOnClick}>
			<div className="flex flex-row gap-2">
				<div className={twMerge('flex flex-col items-center justify-center', useGridLayout ? (useVerticalGridLayout ? 'size-14' : 'size-20') : 'size-18')}>
					<div className={twMerge('rounded-lg bg-component-bg-disabled border border-gray-100/5', useGridLayout ? (useVerticalGridLayout ? 'size-14' : 'size-20') : 'size-18')}>
						<img className={twMerge('rounded-lg', modMetadata.iconURL === null ? 'hidden' : '', useGridLayout ? (useVerticalGridLayout ? 'size-14' : 'size-20') : 'size-18')} src={modMetadata.iconURL ?? MissingLogo} />
					</div>
				</div>
				<div className={twMerge('flex flex-col', useVerticalGridLayout && useGridLayout ? 'justify-center' : '')}>
					<div className="flex flex-row flex-wrap">
						<p className={twMerge('text-fg-primary break-words', useGridLayout ? 'text-lg' : 'text-xl', useGridLayout && !useVerticalGridLayout ? 'max-w-3/5' : '')}>{modMetadata.name}</p>
						{useVerticalGridLayout !== true && <ModTag cluster={cluster} modData={modMetadata} />}
					</div>

					<p className={twMerge(modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary', useGridLayout ? 'text-sm' : 'text-base')}>
						by
						{' '}
						<span className="font-semibold">{modMetadata.author ?? 'UNKNOWN'}</span>
					</p>
					{useVerticalGridLayout !== true && <p className={twMerge('font-normal', modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary', useGridLayout ? 'text-sm' : 'text-base')}>{modMetadata.description ?? 'No Description'}</p>}
				</div>
			</div>
			{useVerticalGridLayout === true && modMetadata.description !== null && <p className={twMerge('font-normal text-fg-secondary', useGridLayout ? 'text-sm' : 'text-base')}>{modMetadata.description}</p>}
		</div>
	);
}
