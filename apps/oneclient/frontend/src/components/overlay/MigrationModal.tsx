import type { ClusterModel } from '@/bindings.gen';
import { Overlay } from '@/components';
import { useCachedImage } from '@/hooks/useCachedImage';
import { bindings } from '@/main';
import { prettifyLoader } from '@/utils/loaders';
import { useToast } from '@/utils/toast';
import { getOnlineClusterForVersion } from '@/utils/versionMap';
import { useCommand } from '@onelauncher/common';
import { Dropdown } from '@onelauncher/common/components';
import { Announcement01Icon } from '@untitled-theme/icons-react';
import { useEffect, useRef, useState } from 'react';

interface MigrationModalProps {
	isOpen: boolean;
	isDebugPreview?: boolean;
	onOpenChange: (open: boolean) => void;
	newVersions: Array<string>;
	sourceClusters: Array<ClusterModel>;
	allClusters: Array<ClusterModel>;
}

export function MigrationModal({
	isOpen,
	isDebugPreview = false,
	onOpenChange,
	newVersions,
	sourceClusters,
	allClusters,
}: MigrationModalProps) {
	const [currentIndex, setCurrentIndex] = useState(0);
	const prevOpen = useRef(isOpen);

	useEffect(() => {
		if (isOpen && !prevOpen.current)
			setCurrentIndex(0);
		prevOpen.current = isOpen;
	}, [isOpen]);

	const currentVersion = newVersions[currentIndex];

	const currentClusters = allClusters.filter(c =>
		c.mc_version === currentVersion && c.last_played === null);

	const defaultSourceId = sourceClusters[sourceClusters.length - 1]?.id ?? null;

	const [targetId, setTargetId] = useState<number | null>(null);
	const [sourceId, setSourceId] = useState<number | null>(defaultSourceId);

	const effectiveTargetId = targetId ?? currentClusters.at(0)?.id ?? null;
	const [isMigrating, setIsMigrating] = useState(false);
	const [fallbackFiles, setFallbackFiles] = useState<Array<string>>([]);

	const toast = useToast();

	const { data: manifest } = useCommand(
		['getVersions'],
		() => bindings.oneclient.getVersions(),
	);

	const onlineCluster = manifest
		? getOnlineClusterForVersion(currentVersion, manifest)
		: undefined;
	const artSrc = useCachedImage(onlineCluster?.art);
	const displayName = onlineCluster?.name ?? currentVersion;

	function advanceOrClose() {
		if (currentIndex < newVersions.length - 1) {
			setCurrentIndex(i => i + 1);
			setTargetId(null);
			setSourceId(defaultSourceId);
		}
		else {
			onOpenChange(false);
		}
	}

	async function migrate() {
		if (sourceId === null || effectiveTargetId === null)
			return;

		if (isDebugPreview) {
			toast({ type: 'info', title: 'Debug preview', message: 'Migration is disabled in debug preview mode.' });
			advanceOrClose();
			return;
		}

		setIsMigrating(true);
		try {
			const result = await bindings.oneclient.copyClusterContent(sourceId, effectiveTargetId);
			if (result.fallback_files.length > 0)
				setFallbackFiles(result.fallback_files);
			else
				toast({ type: 'success', title: 'Migration complete', message: 'Your content has been copied to the new version.' });
			advanceOrClose();
		}
		catch {
			toast({ type: 'error', title: 'Migration failed', message: 'Could not copy content. Please try again.' });
		}
		finally {
			setIsMigrating(false);
		}
	}

	return (
		<>
			<Overlay isDismissable={false} isOpen={isOpen} onOpenChange={() => {}}>
				<Overlay.Dialog className="p-0 overflow-hidden w-[56rem] max-w-[95vw] min-h-[28rem] flex-row items-stretch gap-0">
					{/* Art banner */}
					<div className="relative w-[46%] min-h-[28rem] shrink-0">
						{artSrc
							? (
									<img
										alt={`Minecraft ${displayName}`}
										className="w-full h-full object-cover"
										src={artSrc}
									/>
								)
							: (
									<div className="w-full h-full bg-gradient-to-br from-brand/20 to-page" />
								)}
						<div className="absolute inset-0 bg-gradient-to-t from-page-elevated via-page-elevated/30 to-transparent" />

						<div className="absolute bottom-0 left-0 right-0 p-4">
							<div className="inline-flex items-center gap-1.5 bg-brand text-white text-xs font-semibold px-2.5 py-1 rounded-full mb-2">
								<Announcement01Icon className="size-3" />
								New version
							</div>
							<h2 className="text-2xl font-bold text-white leading-tight">
								{currentVersion}
								{' '}
								is now on OneClient!
							</h2>
						</div>
					</div>

					{/* Migration controls */}
					<div className="flex flex-col gap-4 p-6 w-[54%] justify-center">
						<p className="text-fg-secondary text-sm text-center">
							Copy your content from a previous version to get started.
						</p>

						{newVersions.length > 1 && (
							<p className="text-xs text-fg-secondary text-center">
								Version
								{' '}
								{currentIndex + 1}
								{' '}
								of
								{' '}
								{newVersions.length}
							</p>
						)}

						{currentClusters.length > 1 && (
							<div className="flex flex-col w-full gap-1">
								<p className="text-sm font-medium">Copy to</p>
								<Dropdown
									aria-label="Target version"
									onSelectionChange={key => setTargetId(Number(key))}
									selectedKey={effectiveTargetId}
								>
									{currentClusters.map(c => (
										<Dropdown.Item id={c.id} key={c.id} textValue={`${c.mc_version} ${prettifyLoader(c.mc_loader)}`}>
											{c.mc_version}
											{' '}
											{prettifyLoader(c.mc_loader)}
										</Dropdown.Item>
									))}
								</Dropdown>
							</div>
						)}

						<div className="flex flex-col w-full gap-1">
							<p className="text-sm font-medium">Copy from</p>
							<Dropdown
								aria-label="Source version"
								onSelectionChange={key => setSourceId(Number(key))}
								selectedKey={sourceId}
							>
								{sourceClusters.map(c => (
									<Dropdown.Item id={c.id} key={c.id} textValue={`${c.mc_version} ${prettifyLoader(c.mc_loader)}`}>
										{c.mc_version}
										{' '}
										{prettifyLoader(c.mc_loader)}
									</Dropdown.Item>
								))}
							</Dropdown>
						</div>

						<Overlay.Buttons
							buttons={[
								{
									key: 'migrate',
									children: isMigrating ? 'Migrating...' : 'Migrate',
									color: 'primary',
									isDisabled: sourceId === null || currentClusters.length === 0 || isMigrating,
									onPress: migrate,
								},
							]}
						/>
					</div>
				</Overlay.Dialog>
			</Overlay>

			<Overlay isOpen={fallbackFiles.length > 0} onOpenChange={() => setFallbackFiles([])}>
				<Overlay.Dialog isDismissable>
					<Overlay.Title>Some files could not be updated</Overlay.Title>

					<p className="text-fg-secondary text-sm text-center">
						The following custom files had no compatible version available for the new Minecraft version. They have been copied over as disabled and will not load until you manually enable them.
					</p>

					<ul className="w-full flex flex-col gap-1 max-h-48 overflow-y-auto">
						{fallbackFiles.map(file => (
							<li className="text-sm text-fg-primary bg-component-bg rounded px-2 py-1 font-mono break-all" key={file}>
								{file}
							</li>
						))}
					</ul>

					<Overlay.Buttons
						buttons={[
							{
								key: 'ok',
								children: 'OK',
								color: 'primary',
								slot: 'close',
							},
						]}
					/>
				</Overlay.Dialog>
			</Overlay>
		</>
	);
}
