import type { ManagedPackage, ManagedVersion, ModpackArchive, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { ExternalLink } from '@/components/ExternalLink';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Download01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/cluster/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	const openMods = () => bindings.folders.openCluster(`${cluster.folder_name}/mods`);
	const [mods, setMods] = useState<Array<string>>([]);
	const updateMods = () => {
		(async () => {
			setMods(await bindings.core.getMods(cluster.id));
		})();
	};

	useEffect(updateMods, []);

	return (
		<Tabs defaultValue={bundles[0].manifest.name}>
			<TabList className="gap-6">
				{bundles.map(bundle => <Tab key={bundle.manifest.name} value={bundle.manifest.name}>{(bundle.manifest.name.match(/\[(.*?)\]/)?.[1]) ?? 'UNKNOWN'}</Tab>)}
				<div className="absolute right-4">
					<div className="flex flex-row gap-2">
						<Button onClick={openMods}>Open mods folder</Button>
						{/* <div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1 flex items-center">
							{mods.length} Mods installed
						</div> */}
					</div>
				</div>
			</TabList>

			<TabContent>
				{bundles.map(bundle => (
					<Bundle
						bundleData={bundle}
						key={bundle.manifest.name}
						mods={mods}
						updateMods={updateMods}
					/>
				))}
			</TabContent>
		</Tabs>
	);
}

function Bundle({ bundleData, updateMods, mods }: { bundleData: ModpackArchive; updateMods: () => void; mods: Array<string> }) {
	return (
		<TabPanel value={bundleData.manifest.name}>
			<div className="grid gap-2 grid-cols-1">
				{bundleData.manifest.files.map((file, index) => (
					<ModCard
						file={file}
						key={index}
						mods={mods}
						updateMods={updateMods}
					/>
				))}
			</div>
		</TabPanel>
	);
}

interface ModInfo {
	name: string;
	description: string | null;
	author: string | null;
	iconURL: string | null;
	managed: boolean;
	url: string | null;
}

async function getModAuthor(kind: ModpackFileKind): Promise<string | null> {
	if ('External' in kind)
		return null;
	if (!('Managed' in kind))
		return null;

	const authors = await bindings.core.getUsersFromAuthor(kind.Managed[0].provider, kind.Managed[0].author);
	return authors.map(author => author.username).join(', ');
}

async function getModMetaData(kind: ModpackFileKind): Promise<ModInfo> {
	if ('External' in kind)
		return {
			name: kind.External.name.replaceAll('.jar', ''),
			description: null,
			iconURL: null,
			author: await getModAuthor(kind),
			managed: false,
			url: null,
		};

	return {
		name: kind.Managed[0].name,
		description: kind.Managed[0].short_desc,
		iconURL: kind.Managed[0].icon_url,
		author: await getModAuthor(kind),
		managed: true,
		url: `https://modrinth.com/project/${kind.Managed[0].slug}`,
	};
}

function DownloadFileButton({ kind, updateMods, mods }: { kind: ModpackFileKind; updateMods: () => void; mods: Array<string> }) {
	if (!('Managed' in kind))
		return <></>;
	const [pkg, version] = kind.Managed;
	return (
		<DownloadMod
			mods={mods}
			pkg={pkg}
			updateMods={updateMods}
			version={version}
		/>
	);
}

function DownloadMod({ pkg, version, updateMods, mods }: { pkg: ManagedPackage; version: ManagedVersion; updateMods: () => void; mods: Array<string> }) {
	const { cluster } = Route.useRouteContext();
	const download = useCommandMut(() => bindings.core.downloadPackage(pkg.provider, pkg.id, version.version_id, cluster.id, true));

	const handleDownload = () => {
		(async () => {
			await download.mutateAsync();
			updateMods();
		})();
	};

	return (
		<Button
			className="flex flex-col items-center justify-center"
			color="primary"
			isDisabled={mods.includes(version.files[0].file_name) || !download.isIdle}
			onClick={handleDownload}
			size="iconLarge"
		>
			<Download01Icon />
		</Button>
	);
}

function ModTag({ modData }: { modData: ModInfo }) {
	return (
		<ExternalLink className="no-underline flex flex-row items-center justify-center px-4 rounded-full font-normal bg-component-bg border border-gray-100/5 scale-90 gap-2" href={modData.url ?? undefined} includeIcon={modData.managed}>
			<p>{modData.managed ? 'Modrinth' : 'External'}</p>
		</ExternalLink>
	);
}

function ModCard({ file, updateMods, mods }: { file: ModpackFile; updateMods: () => void; mods: Array<string> }) {
	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: null, description: null, name: 'UNKNOWN', iconURL: null, managed: false, url: null });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file.kind)))();
	}, []);

	return (
		<div className="p-2 rounded-lg mb-2 break-inside-avoid flex flex-col justify-between bg-component-bg border border-gray-100/5">
			<div>
				<div className="flex flex-row gap-2 justify-between">
					<div className="flex flex-row gap-2">
						<div className="size-18 flex flex-col items-center justify-center ">
							<div className="rounded-lg size-16 bg-component-bg-disabled border border-gray-100/5">
								<img className={twMerge('rounded-lg size-16', modMetadata.iconURL === null ? 'hidden' : '')} src={modMetadata.iconURL ?? MissingLogo} />
							</div>
						</div>
						<div className="flex flex-col">
							<div className="flex flex-row gap-2">
								<p className="text-fg-primary text-xl">
									{modMetadata.name}
								</p>
								<ModTag modData={modMetadata} />
							</div>
							<p className={modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary'}>
								by
								{' '}
								<span className="font-semibold">{modMetadata.author ?? 'UNKNOWN'}</span>
							</p>
							<p className={twMerge('font-normal', modMetadata.description === null ? 'text-fg-secondary/25' : 'text-fg-secondary')}>{modMetadata.description ?? 'No Description'}</p>
						</div>
					</div>
					<div className="flex flex-col items-center justify-center pr-2">
						<DownloadFileButton kind={file.kind} mods={mods} updateMods={updateMods} />
					</div>
				</div>
			</div>
		</div>
	);
}
