import type { ClusterModel, ManagedPackage, ManagedVersion, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
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

export function ModCard({ file, cluster, showDownload, onClick, outline }: { file: ModpackFile; cluster: ClusterModel; showDownload?: boolean; onClick?: (file: ModpackFile, modMetadata: ModInfo, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>) => void; outline?: boolean }) {
	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: null, description: null, name: 'LOADING', iconURL: null, managed: false, url: null, id: null });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file.kind)))();
	}, [file]);

	const [showOutline, setShowOutline] = useState<boolean>(outline ?? false);
	const handleOnClick = () => {
		if (onClick)
			onClick(file, modMetadata, setShowOutline);
	};

	return (
		<div className={twMerge('p-2 rounded-lg m-1 break-inside-avoid flex flex-row gap-2 justify-between bg-component-bg border border-gray-100/5', showOutline ? 'outline-2 outline-brand' : '')} onClick={handleOnClick}>
			<div className="flex flex-row gap-2">
				<div className="size-18 flex flex-col items-center justify-center">
					<div className="rounded-lg size-16 bg-component-bg-disabled border border-gray-100/5">
						<img className={twMerge('rounded-lg size-16', modMetadata.iconURL === null ? 'hidden' : '')} src={modMetadata.iconURL ?? MissingLogo} />
					</div>
				</div>
				<div className="flex flex-col">
					<div className="flex flex-row gap-2">
						<p className="text-fg-primary text-xl">{modMetadata.name}</p>
						<ModTag cluster={cluster} modData={modMetadata} />
					</div>
					<p className={modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary'}>
						by
						{' '}
						<span className="font-semibold">{modMetadata.author ?? 'UNKNOWN'}</span>
					</p>
					<p className={twMerge('font-normal', modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary')}>{modMetadata.description ?? 'No Description'}</p>
				</div>
			</div>

			<div className={twMerge('flex-col items-center justify-center pr-2', isManagedMod(modMetadata) && (showDownload === true) ? 'flex' : 'hidden')}>
				{isManagedMod(modMetadata) && (showDownload === true) && (<DownloadModButton cluster={cluster} pkg={modMetadata.pkg} version={modMetadata.version} />)}
			</div>
		</div>
	);
}
