import type { ManagedPackage, ManagedVersion, ModpackArchive, ModpackFile, ModpackFileKind } from '@/bindings.gen';
import MissingLogo from '@/assets/misc/missingLogo.svg';
import { ExternalLink } from '@/components/ExternalLink';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button, Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Download01Icon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';

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
						<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1 flex items-center">
							{mods.length} Mods installed
						</div>
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
	description: string;
	author: string;
	iconURL: string;
	managed: boolean;
}

async function getModAuthor(kind: ModpackFileKind): Promise<string> {
	if ('External' in kind)
		return 'UNKNOWN';
	if (!('Managed' in kind))
		return 'UNKNOWN';

	const authors = await bindings.core.getUsersFromAuthor(kind.Managed[0].provider, kind.Managed[0].author);
	return authors.map(author => author.username).join(', ');
}

function getModIconURL(url?: string | null) {
	return url || MissingLogo;
}

async function getModMetaData(kind: ModpackFileKind): Promise<ModInfo> {
	return {
		name: 'External' in kind
			? kind.External.name.replaceAll('.jar', '')
			: 'Managed' in kind
				? kind.Managed[0].name
				: 'UNKNOWN',
		description: 'External' in kind
			? 'No Description'
			: 'Managed' in kind
				? kind.Managed[0].short_desc
				: 'UNKNOWN',
		author: await getModAuthor(kind),
		iconURL: 'External' in kind
			? getModIconURL()
			: 'Managed' in kind
				? getModIconURL(kind.Managed[0].icon_url)
				: getModIconURL(),
		managed: 'Managed' in kind,
	};
}

function ModrinthVersionButton({ kind }: { kind: ModpackFileKind }) {
	if (!('Managed' in kind))
		return <></>;
	const [pkg, version] = kind.Managed;
	return (
		<ExternalLink className="text-link hover:text-link-hover" href={`https://modrinth.com/project/${pkg.slug}/version/${version.version_id}`} includeIcon>
			{version.display_name}
		</ExternalLink>
	);
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

function ModCard({ file, updateMods, mods }: { file: ModpackFile; updateMods: () => void; mods: Array<string> }) {
	const [modMetadata, setModMetadata] = useState<ModInfo>({ author: 'UNKNOWN', description: 'UNKNOWN', name: 'UNKNOWN', iconURL: getModIconURL(), managed: false });
	useEffect(() => {
		(async () => setModMetadata(await getModMetaData(file.kind)))();
	}, []);

	return (
		<div className="p-2 rounded-lg mb-2 break-inside-avoid flex flex-col justify-between bg-component-bg border border-gray-100/5">
			<div>
				<div className="flex flex-row gap-2 justify-between">
					<div className="flex flex-row gap-2">
						<div className="size-18 flex flex-col items-center justify-center">
							<img className="rounded-lg size-16" src={modMetadata.iconURL} />
						</div>
						<div className="flex flex-col">
							<div className="flex flex-row gap-2">
								<p className="text-fg-primary text-xl">
									{modMetadata.name}
									{' '}
									{modMetadata.managed
										? (
												<>
													(
													<ModrinthVersionButton kind={file.kind} />
													)
												</>
											)
										: <></>}
									{' '}
								</p>
								<div className="flex flex-col items-center justify-center px-4 rounded-full text-fg-secondary font-normal bg-component-bg border border-gray-100/5">
									<p>{modMetadata.managed ? 'Modrinth' : 'SkyClient'}</p>
								</div>
							</div>
							<p className="text-fg-secondary">
								by
								{' '}
								<span className="font-semibold">{modMetadata.author}</span>
							</p>
							<p className="text-fg-secondary font-normal">{modMetadata.description}</p>
						</div>
					</div>
					<div className="flex flex-col items-center justify-center gap-2">
						<DownloadFileButton kind={file.kind} mods={mods} updateMods={updateMods} />
					</div>
				</div>
			</div>
		</div>
	);
}
