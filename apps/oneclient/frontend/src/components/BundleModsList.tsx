import type { ClusterModel, ManagedPackage, ManagedVersion, ModpackArchive, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';
import { Download01Icon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export function BundleModsList({ cluster }: { cluster: ClusterModel }) {
	const { data: bundles } = useCommandSuspense(['getBundlesFor', cluster.id], () => bindings.oneclient.getBundlesFor(cluster.id));

	const [mods, setMods] = useState<Array<string>>([]);
	const updateMods = () => {
		(async () => {
			setMods(await bindings.core.getMods(cluster.id));
		})();
	};

	if (bundles.length === 0)
		return (
			<>
				<p>No bundles found {cluster.name}</p>
			</>
		);

	return (
		<Tabs defaultValue={bundles[0].manifest.name}>
			<TabList className="gap-6">
				{bundles.map(bundle => <Tab key={bundle.manifest.name} value={bundle.manifest.name}>{(bundle.manifest.name.match(/\[(.*?)\]/)?.[1]) ?? 'LOADING'}</Tab>)}
				<div className="absolute right-4">
					<div className="bg-fg-primary rounded-xl text-brand text-sm px-4 py-2">
						{mods.length} Mods Installed
					</div>
				</div>
			</TabList>

			<TabContent>
				{bundles.map(bundle => (
					<Bundle
						bundleData={bundle}
						cluster={cluster}
						key={bundle.manifest.name}
						mods={mods}
						updateMods={updateMods}
					/>
				))}
			</TabContent>
		</Tabs>
	);
}

function Bundle({ bundleData, updateMods, mods, cluster }: { bundleData: ModpackArchive; updateMods: () => void; mods: Array<string>; cluster: ClusterModel }) {
	return (
		<TabPanel value={bundleData.manifest.name}>
			<OverlayScrollbarsComponent>
				<div className="grid gap-2 grid-cols-1 h-112">
					{bundleData.manifest.files.map((file, index) => (
						<ModCard
							cluster={cluster}
							file={file}
							key={index}
							mods={mods}
							updateMods={updateMods}
						/>
					))}
				</div>
			</OverlayScrollbarsComponent>
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
	id: string | null;
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
	};
}

function DownloadFileButton({ kind, updateMods, mods, cluster }: { kind: ModpackFileKind; updateMods: () => void; mods: Array<string>; cluster: ClusterModel }) {
	if (!('Managed' in kind))
		return <></>;
	const [pkg, version] = kind.Managed;
	return (
		<DownloadMod
			cluster={cluster}
			mods={mods}
			pkg={pkg}
			updateMods={updateMods}
			version={version}
		/>
	);
}

function DownloadMod({ pkg, version, updateMods, mods, cluster }: { pkg: ManagedPackage; version: ManagedVersion; updateMods: () => void; mods: Array<string>; cluster: ClusterModel }) {
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

function ModTag({ modData, cluster }: { modData: ModInfo; cluster: ClusterModel }) {
	return (
		<Link
			className="flex flex-row items-center justify-center px-4 rounded-full font-normal bg-component-bg border border-gray-100/5 scale-90"
			search={{ provider: 'Modrinth', packageId: modData.id ?? '', clusterId: cluster.id }}
			to="/app/cluster/browser/package"
		>
			<p>{modData.managed ? 'Modrinth' : 'External'}</p>
		</Link>
	);
}

function ModCard({ file, updateMods, mods, cluster }: { file: ModpackFile; updateMods: () => void; mods: Array<string>; cluster: ClusterModel }) {
	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: null, description: null, name: 'LOADING', iconURL: null, managed: false, url: null, id: null });
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
					<div className="flex flex-col items-center justify-center pr-2">
						<DownloadFileButton
							cluster={cluster}
							kind={file.kind}
							mods={mods}
							updateMods={updateMods}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
