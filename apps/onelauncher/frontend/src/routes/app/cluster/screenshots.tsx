import Modal from '@/components/overlay/Modal';
import ScrollableContainer from '@/components/ScrollableContainer';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Show } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { convertFileSrc } from '@tauri-apps/api/core';
import { dataDir, join } from '@tauri-apps/api/path';
import { useEffect, useState } from 'react';
import { Pressable } from 'react-aria-components';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/screenshots')({
	component: RouteComponent,
});

function RouteComponent() {
	const { id } = Route.useSearch();
	const cluster = useCommand('getClusterById', () => bindings.core.getClusterById(Number(id.toString()) as unknown as bigint));
	const result = useCommand('getScreenshots', () => bindings.core.getScreenshots(Number(id.toString()) as unknown as bigint));

	return (
		<Sidebar.Page>
			<h1>Screenshots</h1>
			<ScrollableContainer>
				<div className="h-full">
					<Show
						fallback={<div className="text-border/400">No screenshots found. Press F2 in game to take a screenshot!</div>}
						// @ts-ignore idk
						when={result.data.length > 0}
					>
						<div className="grid grid-cols-[repeat(auto-fill,minmax(350px,1fr))] w-full transform-gpu gap-2">
							{result.data.map(data => (
								<ScreenshotEntry cluster_path={cluster.data?.folder_name || ''} key={data} path={data} />
							))}
						</div>
					</Show>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

function ScreenshotEntry({
	path,
	cluster_path,
}: {
	path: string;
	cluster_path: string;
}) {
	const [imagePath, setImagePath] = useState<string>('');

	useEffect(() => {
		async function loadImagePath() {
			const screenshotsDir = await join(await dataDir(), 'OneLauncher', 'clusters', cluster_path, 'screenshots');
			const fullPath = await join(screenshotsDir, path);
			setImagePath(convertFileSrc(fullPath));
		}

		loadImagePath().catch(console.error);
	}, [cluster_path, path]);

	return (
		<>
			<Modal.Trigger>
				<Pressable>
					<div className="rounded-md overflow-hidden bg-component-bg-pressed outline-0 h-64 flex flex-col relative">
						<img
							alt={path}
							className="aspect-video w-full rounded-lg"
							src={imagePath}
						/>
					</div>
				</Pressable>
				<Modal className="w-full">
					<img className="rounded-xl" src={imagePath} />
				</Modal>
			</Modal.Trigger>
		</>
	);
}
