import type { ClusterModel, ManagedPackage, ManagedVersion, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { createContext, useContext, useEffect, useState } from 'react';
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

interface ModInfoManged extends ModInfo {
	pkg: ManagedPackage;
	version: ManagedVersion;
}

async function getModAuthor(kind: ModpackFileKind, useVerticalGridLayout?: boolean): Promise<string | null> {
	if ('External' in kind)
		return null;
	if (!('Managed' in kind))
		return null;

	const authors = await bindings.core.getUsersFromAuthor(kind.Managed[0].provider, kind.Managed[0].author);
	if (useVerticalGridLayout) {
		const truncated = authors.slice(0, 2);
		const extra = authors.length - truncated.length;
		const names = truncated.map(author => author.username).join(', ');
		if (extra > 0)
			return `${names} and ${extra} more`;
		else
			return names;
	}
	return authors.map(author => author.username).join(', ');
}

async function getModMetaData(kind: ModpackFileKind, useVerticalGridLayout?: boolean): Promise<ModInfo | ModInfoManged> {
	if ('External' in kind)
		return {
			name: kind.External.name.replaceAll('.jar', ''),
			description: null,
			iconURL: null,
			author: await getModAuthor(kind, useVerticalGridLayout),
			managed: false,
			url: null,
			id: null,
		};

	return {
		name: kind.Managed[0].name,
		description: kind.Managed[0].short_desc,
		iconURL: kind.Managed[0].icon_url,
		author: await getModAuthor(kind, useVerticalGridLayout),
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

interface ModCardProps {
	file: ModpackFile;
	cluster: ClusterModel;
}

export type onClickOnMod = (file: ModpackFile, setShowOutline: React.Dispatch<React.SetStateAction<boolean>>, setShowBlueBackground: React.Dispatch<React.SetStateAction<boolean>>) => void;
export interface ModCardContextApi {
	showModDownloadButton?: boolean;
	onClickOnMod?: onClickOnMod;
	showOutlineOnCard?: boolean;
	showBlueBackgroundOnCard?: boolean;
	useVerticalGridLayout?: boolean;
}

export const ModCardContext = createContext<ModCardContextApi | null>(null);
export function useModCardContext() {
	const ctx = useContext(ModCardContext);

	if (!ctx)
		throw new Error('useModCardContext must be used within a ModCardContext.Provider');

	return ctx;
}

export function ModCard({ file, cluster }: ModCardProps) {
	const { showModDownloadButton, onClickOnMod, showOutlineOnCard, showBlueBackgroundOnCard, useVerticalGridLayout } = useModCardContext();

	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: null, description: null, name: 'LOADING', iconURL: null, managed: false, url: null, id: null });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file.kind, useVerticalGridLayout)))();
	}, [file, useVerticalGridLayout]);

	const [showOutline, setShowOutline] = useState<boolean>(showOutlineOnCard ?? false);
	const [showBlueBackground, setShowBlueBackground] = useState<boolean>(showBlueBackgroundOnCard ?? false);
	const handleOnClick = () => {
		if (onClickOnMod)
			onClickOnMod(file, setShowOutline, setShowBlueBackground);
	};

	const { setting } = useSettings();
	const useGridLayout = setting('mod_list_use_grid');

	return (
		<div className={twMerge('rounded-lg m-1 break-inside-avoid flex bg-component-bg border border-gray-100/5', useVerticalGridLayout && useGridLayout ? 'p-1' : 'p-2 gap-2 justify-between', useGridLayout ? 'flex-col' : 'flex-row', showOutline ? 'outline-2 outline-brand' : '', showBlueBackground ? 'bg-brand/20' : '')} onClick={handleOnClick}>
			<div className="flex flex-row gap-2">
				<div className={twMerge('flex flex-col items-center justify-center', useGridLayout ? (useVerticalGridLayout ? 'size-14' : 'size-20') : 'size-18')}>
					<div className={twMerge('rounded-lg bg-component-bg-disabled border border-gray-100/5', useGridLayout ? (useVerticalGridLayout ? 'size-12' : 'size-18') : 'size-16')}>
						<img className={twMerge('rounded-lg', modMetadata.iconURL === null ? 'hidden' : '', useGridLayout ? (useVerticalGridLayout ? 'size-12' : 'size-18') : 'size-16')} src={modMetadata.iconURL ?? MissingLogo} />
					</div>
				</div>
				<div className={twMerge('flex flex-col', useVerticalGridLayout && useGridLayout ? 'justify-center' : '')}>
					<div className="flex flex-row flex-wrap gap-2">
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
			{useVerticalGridLayout === true && modMetadata.description !== null && <p className={twMerge('font-normal text-fg-secondary p-2', useGridLayout ? 'text-sm' : 'text-base')}>{modMetadata.description}</p>}

			{isManagedMod(modMetadata) && showModDownloadButton === true && (
				<div className="flex flex-col items-center justify-center pr-2">
					<DownloadModButton cluster={cluster} pkg={modMetadata.pkg} version={modMetadata.version} />
				</div>
			)}
		</div>
	);
}
