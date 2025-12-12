import type { GameLoader } from '@/bindings.gen';
import { GameBackground, LaunchButton, SheetPage } from '@/components';
import { bindings } from '@/main';
import useClusterStore from '@/stores/clusterStore';
import { prettifyLoader } from '@/utils/loaders';
import { getVersionInfo, getVersionInfoOrDefault, parseMcVersion } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { Button, Dropdown } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { ArrowRightIcon } from '@untitled-theme/icons-react';
import { useCallback, useMemo, useState } from 'react';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/app/clusters')({
	component: RouteComponent,
});

function RouteComponent() {
	const prevMinorVersions = useClusterStore(state => state.minorVersions);
	const setMinorVersion = useClusterStore(state => state.setMinorVersion);
	const modLoaders = useClusterStore(state => state.modLoaders);
	const setModLoader = useClusterStore(state => state.setModLoader);

	const navigate = Route.useNavigate();

	const { data: clusters } = useCommandSuspense(['getClustersGroupedByMajor'], bindings.oneclient.getClustersGroupedByMajor);

	const loadMinorVersion = useCallback((major: number) => {
		const existing = prevMinorVersions.find(v => v.major === major);
		if (existing)
			return existing.minor;
		return undefined;
	}, [prevMinorVersions]);

	const [majorVersion, setMajorVersion] = useState<number>(() => {
		const keys = Object.keys(clusters);
		if (keys.length === 0)
			return 0;

		return Number.parseInt(keys[0]);
	});

	const [activeMinorVersion, setActiveMinorVersion] = useState<number | undefined>();

	const minorVersions = useMemo(() => {
		const list = clusters[majorVersion];
		if (!list || list.length === 0)
			return [];

		const versions = list.map(c => parseMcVersion(c.mc_version)?.minor).filter((v): v is number => v !== undefined);
		const sorted = Array.from(new Set(versions)).sort((a, b) => a - b);

		// attempt to load the minor version from localStorage
		const storedMinor = loadMinorVersion(majorVersion) || sorted[0];
		setActiveMinorVersion(storedMinor);

		return sorted;
	}, [clusters, loadMinorVersion, majorVersion]);

	const modLoader = useMemo(() => {
		if (!activeMinorVersion)
			return undefined;

		return modLoaders[`${majorVersion}.${activeMinorVersion}`];
	}, [modLoaders, majorVersion, activeMinorVersion]);

	const loaders = useMemo(() => {
		const list = clusters[majorVersion];
		if (!list || list.length === 0)
			return [];

		const loadersSet: Set<GameLoader> = new Set();
		for (const cluster of list)
			if (cluster.mc_loader !== 'vanilla')
				loadersSet.add(cluster.mc_loader);

		return Array.from(loadersSet).sort();
	}, [clusters, majorVersion]);

	const cluster = useMemo(() => {
		const list = clusters[majorVersion];
		if (!activeMinorVersion)
			return list?.[0];

		return list?.find((c) => {
			const version = c.mc_version.endsWith(activeMinorVersion.toString());
			const loaderCheck = modLoader ? c.mc_loader === modLoader : true;
			return version && loaderCheck;
		});
	}, [clusters, majorVersion, activeMinorVersion, modLoader]);

	const versionInfo = useMemo(() => getVersionInfoOrDefault(cluster?.mc_version), [cluster]);

	const view = useCallback(() => {
		if (!cluster)
			return;

		navigate({
			to: `/app/cluster/overview`,
			search: {
				clusterId: cluster.id,
			},
		});
	}, [cluster, navigate]);

	if (!cluster)
		return (
			<SheetPage
				headerLarge={<HeaderLarge />}
				headerSmall={<HeaderSmall />}
			>
				<div className="relative flex flex-row gap-4">
					<SheetPage.Content className="sticky top-8 w-86 h-min flex flex-col p-2 gap-2 outline outline-ghost-overlay">
						<p>Something wen't wrong while getting a cluster</p>
						<p>Please reload and if this does not work please contact support</p>
					</SheetPage.Content>
				</div>
			</SheetPage>
		);

	return (
		<SheetPage
			headerLarge={<HeaderLarge />}
			headerSmall={<HeaderSmall />}
		>
			<div className="relative flex flex-row gap-4">
				<div className="flex flex-col flex-1">

					<div className="grid grid-cols-2 2xl:grid-cols-3 gap-4">
						{Object.keys(clusters).map((majorStr) => {
							const major = Number.parseInt(majorStr, 10);
							const isSelected = major === majorVersion;
							const info = getVersionInfo(major);

							if (!info)
								return undefined;

							return (
								<ClusterEntry
									isSelected={isSelected}
									key={major}
									major_version={major}
									onClick={() => setMajorVersion(major)}
									tags={[...new Set(clusters[major]?.flatMap(c => prettifyLoader(c.mc_loader)))]}
								/>
							);
						})}
					</div>
				</div>

				<SheetPage.Content className="sticky top-8 w-86 h-min flex flex-col p-2 gap-2 outline outline-ghost-overlay">
					<GameBackground className="aspect-video w-full rounded-xl outline-2 outline-ghost-overlay" name={versionInfo.backgroundName} />

					<div className="flex flex-col px-4 pt-2 pb-4 gap-2">
						<h2 className="text-xxl font-medium">Version {cluster.mc_version}</h2>
						<p className="text-sm text-fg-secondary">{versionInfo.longDescription}</p>

						{minorVersions.length > 1 && (
							<div className="flex flex-row items-center justify-between mt-3">
								<p>Minor Version</p>

								<Dropdown
									aria-label="Minor Version Dropdown"
									onSelectionChange={(selected) => {
										setActiveMinorVersion(Number(selected));
										setMinorVersion(majorVersion, Number(selected));
									}}
									selectedKey={activeMinorVersion}
								>
									{minorVersions.map((minorVersion) => {
										const fullVer = `${versionInfo.prettyName}.${minorVersion}`;
										return (
											<Dropdown.Item
												id={minorVersion}
												key={minorVersion}
												textValue={fullVer}
											>
												{fullVer}
											</Dropdown.Item>
										);
									})}
								</Dropdown>
							</div>
						)}

						{loaders.length > 1 && (
							<div className="flex flex-row items-center justify-between mb-2">
								<p>Mod Loader</p>
								<Dropdown
									aria-label="Mod Loader Dropdown"
									onSelectionChange={(selected) => {
										if (!activeMinorVersion)
											return;

										setModLoader(`${majorVersion}.${activeMinorVersion}`, selected as GameLoader);
									}}
									selectedKey={activeMinorVersion ? (modLoaders[`${majorVersion}.${activeMinorVersion}`] ?? loaders[0]) : loaders[0]}
								>
									{loaders.map(loader => (
										<Dropdown.Item
											id={loader}
											key={loader}
										>
											{prettifyLoader(loader)}
										</Dropdown.Item>
									))}
								</Dropdown>
							</div>
						)}

						<div className="w-full flex flex-row gap-4 mt-2">
							<LaunchButton className="flex-1" cluster={cluster} size="large" />

							<Button
								className="flex-1"
								color="secondary"
								onPress={view}
								size="large"
							>
								View
								<ArrowRightIcon />
							</Button>
						</div>

					</div>
				</SheetPage.Content>
			</div>
		</SheetPage>
	);
}

function HeaderLarge() {
	return (
		<div className="flex flex-row justify-between items-end gap-8">
			<div className="flex flex-col">
				<h1 className="text-3xl font-semibold">Clusters</h1>
				<p className="text-md font-medium text-fg-secondary">Something something in corporate style fashion about picking your preferred gamemodes and versions and optionally loader so that oneclient can pick something for them</p>
			</div>
		</div>
	);
}

function HeaderSmall() {
	return (
		<h1 className="text-2lg h-full font-medium">Clusters</h1>
	);
}

function ClusterEntry({
	isSelected,
	major_version,
	onClick,
	tags,
}: {
	isSelected: boolean;
	major_version: number;
	onClick: () => unknown;
	tags: Array<string>;
}) {
	const info = getVersionInfo(major_version);

	if (!info)
		return undefined;

	return (
		<div className={twMerge('flex flex-col justify-between relative aspect-video transition-[filter] px-4', !isSelected && 'brightness-70 grayscale-25 hover:brightness-80 hover:grayscale-0')} key={major_version} onClick={onClick}>
			<GameBackground className={twMerge('opacity-90 absolute w-full h-full -z-10 rounded-md transition-[outline] outline-2', isSelected ? 'outline-brand' : 'outline-ghost-overlay')} name={info.backgroundName} />
			<div
				className="absolute -z-10 top-0 left-0 w-full h-full rounded-md overflow-hidden " style={{
					background: 'radial-gradient(48.93% 47.95% at 49.92% 28.42%, rgba(0, 0, 0, 0.00) 0%, rgba(0, 0, 0, 0.30) 52.26%, rgba(0, 0, 0, 0.60) 100%)',
				}}
			>
			</div>

			<div className="flex flex-row flex-wrap gap-2 mt-3">
				{info.tags.concat(tags).map(tag => (
					<div className="bg-fg-primary text-brand px-2 text-sm py-0.5 rounded-full" key={tag}>
						{tag}
					</div>
				))}
			</div>

			<h3 className="text-fg-primary text-2xl font-semibold mb-2">Version {info.prettyName}</h3>
		</div>
	);
}
