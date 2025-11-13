import type { ClusterModel, ManagedPackage, ManagedVersion, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';
import { DownloadModButton, ModTag } from '.';

export interface ModInfo {
	name: string;
	description: string | null;
	author: string | null;
	iconURL: string | null;
	managed: boolean;
	url: string | null;
	id: string | null;
}

export interface ModInfoManged extends ModInfo {
	pkg: ManagedPackage;
	version: ManagedVersion;
}

async function getModAuthor(kind: ModpackFileKind): Promise<string | null> {
	if ('External' in kind)
		return null;
	if (!('Managed' in kind))
		return null;

	const authors = await bindings.core.getUsersFromAuthor(kind.Managed[0].provider, kind.Managed[0].author);
	return authors.map(author => author.username).join(', ');
}

export async function getModMetaData(kind: ModpackFileKind): Promise<ModInfo | ModInfoManged> {
	if ('External' in kind)
		return {
			name: kind.External.name.replaceAll('.jar', ''),
			description: null,
			iconURL: null,
			author: await getModAuthor(kind),
			managed: false,
			url: null,
			id: null,
		};

	return {
		name: kind.Managed[0].name,
		description: kind.Managed[0].short_desc,
		iconURL: kind.Managed[0].icon_url,
		author: await getModAuthor(kind),
		managed: true,
		url: `https://modrinth.com/project/${kind.Managed[0].slug}`,
		id: kind.Managed[0].id,
		pkg: kind.Managed[0],
		version: kind.Managed[1],
	};
}

export function isManagedMod(mod: ModInfo | ModInfoManged): mod is ModInfoManged {
	return mod.managed === true;
}

export function ModCard({ file, cluster, showDownload, onClick, outline, blueBackground, useVerticalGridLayout }: { file: ModpackFile; cluster: ClusterModel; showDownload?: boolean; onClick?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>, setShowBlueBackground: React.Dispatch<React.SetStateAction<boolean>>) => void; outline?: boolean; blueBackground?: boolean; useVerticalGridLayout?: boolean }) {
	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: null, description: null, name: 'LOADING', iconURL: null, managed: false, url: null, id: null });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file.kind)))();
	}, [file]);

	const [showOutline, setShowOutline] = useState<boolean>(outline ?? false);
	const [showBlueBackground, setShowBlueBackground] = useState<boolean>(blueBackground ?? false);
	const handleOnClick = () => {
		if (onClick)
			onClick(file, modMetadata, setShowOutline, setShowBlueBackground);
	};

	const { setting } = useSettings();
	const grid = setting('mod_list_use_grid');

	return (
		<div className={twMerge('rounded-lg m-1 break-inside-avoid flex bg-component-bg border border-gray-100/5', useVerticalGridLayout && grid ? "p-1" : "p-2 gap-2 justify-between", grid ? 'flex-col' : 'flex-row', showOutline ? 'outline-2 outline-brand' : '', showBlueBackground ? 'bg-brand/20' : '')} onClick={handleOnClick}>
			<div className="flex flex-row gap-2">
				<div className={twMerge('flex flex-col items-center justify-center', grid ? (useVerticalGridLayout ? "size-14" : 'size-20') : 'size-18')}>
					<div className={twMerge('rounded-lg bg-component-bg-disabled border border-gray-100/5', grid ? (useVerticalGridLayout ? "size-12" : 'size-18') : 'size-16')}>
						<img className={twMerge('rounded-lg', modMetadata.iconURL === null ? 'hidden' : '', grid ? (useVerticalGridLayout ? 'size-12' : 'size-18') : 'size-16')} src={modMetadata.iconURL ?? MissingLogo} />
					</div>
				</div>
				<div className="flex flex-col">
					<div className="flex flex-row flex-wrap gap-2">
						<p className={twMerge('text-fg-primary break-words', grid ? 'text-lg' : 'text-xl', grid && !useVerticalGridLayout ? "max-w-3/5" : "")}>{modMetadata.name}</p>
						{useVerticalGridLayout !== true && <ModTag cluster={cluster} modData={modMetadata} />}
					</div>

					<p className={twMerge(modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary', grid ? 'text-sm' : 'text-base')}>
						by
						{' '}
						<span className="font-semibold">{modMetadata.author ?? 'UNKNOWN'}</span>
					</p>
					{useVerticalGridLayout !== true && <p className={twMerge('font-normal', modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary', grid ? 'text-sm' : 'text-base')}>{modMetadata.description ?? 'No Description'}</p>}
				</div>
			</div>
			{useVerticalGridLayout === true && <p className={twMerge('font-normal', modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary', grid ? 'text-sm' : 'text-base')}>{modMetadata.description ?? 'No Description'}</p>}

			{isManagedMod(modMetadata) && showDownload === true && (
				<div className="flex flex-col items-center justify-center pr-2">
					<DownloadModButton cluster={cluster} pkg={modMetadata.pkg} version={modMetadata.version} />
				</div>
			)}
		</div>
	);
}
