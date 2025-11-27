import type { ClusterModel, ModpackFile, ModpackFileKind } from '@/bindings.gen';
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
	showModDownloadButton?: boolean;
	onClickOnMod?: onClickOnMod;
	useVerticalGridLayout?: boolean;
	mods?: Array<ModpackFile>;
}

export const ModCardContext = createContext<ModCardContextApi | null>(null);
export function useModCardContext() {
	const ctx = useContext(ModCardContext);

	if (!ctx)
		throw new Error('useModCardContext must be used within a ModCardContext.Provider');

	return ctx;
}

export function ModCard({ file, cluster }: ModCardProps) {
	const { showModDownloadButton, onClickOnMod, useVerticalGridLayout, mods } = useModCardContext();

	const [modMetadata, setModMetadata] = useState<ModInfo>({ name: 'LOADING', description: null, author: null, iconURL: null, url: null, managed: false, packageSlug: null });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file, useVerticalGridLayout)))();
	}, [file, useVerticalGridLayout]);

	const [isSelected, setSelected] = useState(mods?.includes(file) ?? false);
	useEffect(() => {
		setSelected(mods?.includes(file) ?? false);
	}, [mods, file]);
	const handleOnClick = () => {
		if (onClickOnMod)
			onClickOnMod(file);
	};

	const { setting } = useSettings();
	const useGridLayout = setting('mod_list_use_grid');

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

			{showModDownloadButton === true && (
				<div className={twMerge('flex flex-col items-center justify-center', useGridLayout ? '' : 'pr-2')}>
					<DownloadModButton cluster={cluster} file={file} />
				</div>
			)}
		</div>
	);
}
