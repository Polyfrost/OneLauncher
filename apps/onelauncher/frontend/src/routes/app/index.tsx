import type { ClusterModel } from '@/bindings.gen';
import DefaultBanner from '@/assets/images/default_banner.png';
import DefaultInstancePhoto from '@/assets/images/default_instance_cover.jpg';
import { NewClusterCreate } from '@/components/launcher/cluster/ClusterCreation';
import { useRecentCluster } from '@/hooks/useCluster';
import { bindings } from '@/main';
import { formatAsDuration, upperFirst } from '@/utils';
import { useCommand, useCommandMut } from '@onelauncher/common';
import { Button, ContextMenu, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { convertFileSrc } from '@tauri-apps/api/core';
import { dataDir, join } from '@tauri-apps/api/path';
import { open } from '@tauri-apps/plugin-dialog';
import { openPath } from '@tauri-apps/plugin-opener';
import { CheckIcon, PlayIcon } from '@untitled-theme/icons-react';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { useEffect, useRef, useState } from 'react';

export const Route = createFileRoute('/app/')({
	component: RouteComponent,
});

/*
Please note this route has a very big issue related to scrolling
and i am very angry rn so i will not be fixing it rn

hey future sassan here i guess the issue is solved idk

hey future sassan here i guess the issue is solved idk
*/
function RouteComponent() {
	const result = useCommand('getClusters', bindings.core.getClusters);

	return (
		<div className="h-full flex flex-col gap-y-4 text-fg-primary">
			<Banner />

			<div className="flex flex-row items-center justify-between">
				<NewClusterCreate />
			</div>

			<div className="flex flex-col">
				<ClusterGroup clusters={result.data} isFetching={result.isFetching} />
			</div>
		</div>
	);
}

function Banner() {
	/**
	 * If there are any clusters, display the most recent cluster name + some statistics as the "description".
	 * The background would prioritise
	 * any screenshots taken from the cluster, if there are none, it would display the cluster cover if it has been set.
	 * The button would launch the cluster.
	 *
	 * If there are no clusters, display a generic background with the button action creating a new cluster.
	 */
	const cluster = useRecentCluster();

	// const launch = useLaunchCluster(() => cluster()?.uuid);
	// const navigate = useNavigate();

	const image = () => {
		const url = cluster?.icon_url;

		if (!url)
			return DefaultBanner;

		return convertFileSrc(url);
	};

	return (
		<div className="relative h-52 min-h-52 w-full overflow-hidden rounded-xl border border-component-border">
			<div className="absolute h-52 min-h-52 w-full">
				<div className="linearBlur after:right-1/3">
					<img
						alt="Default Banner"
						className="top-0 left-0 h-full rounded-xl w-full object-cover"
						onError={(e) => {
							(e.target as HTMLImageElement).src = DefaultBanner;
						}}
						src={image()}
					/>
				</div>
			</div>

			<div className="relative z-10 h-full flex flex-col items-start justify-between px-8 py-6">
				<div className="theme-OneLauncher-Dark flex flex-col gap-y-2 text-fg-primary">
					<h1 className="text-2xl font-medium text-shadow-black text-shadow-2xs">{cluster?.name || 'Create a cluster'}</h1>
					<Show when={cluster !== undefined}>
						<p>
							You've played
							{' '}
							<strong>
								{cluster?.mc_version}
								{' '}
								{upperFirst(cluster?.mc_loader || 'Unknown')}
							</strong>
							{' '}
							for
							{' '}
							<strong>{formatAsDuration(cluster?.overall_played || 0)}</strong>
							.
						</p>
					</Show>
				</div>
				<div className="w-full flex flex-row items-end justify-between">
					<div className="flex flex-row items-center gap-x-4">
						{/* <Show when={cluster() !== undefined}>
							<Button
								buttonStyle="primary"
								children={`Launch ${cluster()!.meta.mc_version}`}
								iconLeft={<PlayIcon />}
								onClick={launch}
							/>
							<Button
								buttonStyle="iconSecondary"
								children={<Settings01Icon />}
								className="theme-OneLauncher-Dark bg-op-10!"
								onClick={() => ClusterRoot.open(navigate, cluster()!.uuid)}
							/>
						</Show> */}
					</div>
					<div className="flex flex-row gap-x-2">
						{/* TODO: These tags */}
						{/* <Tag iconLeft={<OneConfigLogo />} />
						<Tag iconLeft={<CheckIcon />}>Verified</Tag> */}
					</div>
				</div>
			</div>
		</div>
	);
}

interface ClusterGroupProps {
	clusters: Array<ClusterModel> | undefined;
	isFetching?: boolean;
}

function ClusterGroup({
	isFetching,
	clusters,
}: ClusterGroupProps) {
	if (isFetching)
		return (
			<div className="flex items-center justify-center h-full">
				<div className="w-8 h-8 border-4 border-brand rounded-full border-t-transparent animate-spin" />
			</div>
		);

	return (
		<div className="h-full w-full">
			<OverlayScrollbarsComponent
				className="h-full w-full"
			>
				<div className="grid grid-cols-4 gap-4 max-h-96 2xl:grid-cols-6 pb-4">
					{clusters?.map(data => (
						<ClusterCard key={data.id} {...data} />
					))}
				</div>
			</OverlayScrollbarsComponent>
		</div>
	);
}

function ClusterCard({
	id,
	name,
	mc_loader,
	mc_version,
}: ClusterModel) {
	const launch = useCommandMut(() => bindings.core.launchCluster(id, null));
	const ref = useRef<HTMLDivElement>(null);
	const [isOpen, setOpen] = useState(false);
	const navigate = useNavigate({ from: '/app' });
	const [launcherDir, setLauncherDir] = useState('');
	const cluster = useCommand(`getClusterById-${id}`, () => bindings.core.getClusterById(id));
	const [newCover, setNewCover] = useState<string>(cluster.data?.icon_url as string);
	const [newName, setNewName] = useState<string>(cluster.data?.name as string);
	const [edit, setEdit] = useState(false);

	const handleLaunch = () => {
		launch.mutate();

		if (launch.error)
			console.error(launch.error.message);
	};

	useEffect(() => {
		(async () => {
			setLauncherDir(await join(await dataDir(), 'OneLauncher', 'clusters', cluster.data!.folder_name));
		})();
	}, [cluster.data?.folder_name]);

	const openClusterDir = async () => {
		openPath(launcherDir);
	};

	const editing = useCommand(`updateClusterById-${id}`, () => bindings.core.updateClusterById(id, {
		icon_url: newCover,
		name: newName,
	}), {
		enabled: false,
		subscribed: false,
	});

	async function launchFilePicker() {
		const selected = await open({
			multiple: false,
			directory: false,
			filters: [{
				name: 'Image',
				extensions: ['png', 'jpg', 'jpeg', 'webp'],
			}],
		});

		if (!selected)
			return;

		setNewCover(selected);
	}

	useEffect(() => {
		if ((newCover && newCover !== cluster.data?.icon_url) || (newName && newName !== cluster.data?.name))
			editing.refetch();
	}, [newCover, newName]);

	const image = () => {
		if (newCover && newCover !== cluster.data?.icon_url)
			if (newCover.includes('\\') || newCover.includes('/')) {
				console.warn('Using preview image:', newCover);
				return convertFileSrc(newCover);
			}

		const url = cluster.data?.icon_url;

		if (!url)
			return DefaultInstancePhoto;

		return convertFileSrc(url);
	};

	function updateName(name: string) {
		if (name.length > 30 || name.length <= 0)
			return;

		setNewName(name);
	}

	return (
		<div ref={ref}>
			<div
				className="group relative h-[152px] flex flex-col rounded-xl border border-component-border/5 bg-component-bg active:bg-component-bg-pressed hover:bg-component-bg-hover"
			>
				<div className="relative flex-1 overflow-hidden rounded-t-xl">
					<div
						className="absolute h-full w-full transition-transform group-hover:!scale-110"
					>
						<img
							className="h-full w-full object-cover"
							onError={(e) => {
								(e.target as HTMLImageElement).src = DefaultInstancePhoto;
							}}
							src={image()}
						/>
					</div>
				</div>
				<div className="z-10 flex flex-row items-center justify-between gap-x-3 p-3">
					<div className="h-full flex flex-col gap-1.5 overflow-hidden">
						<Show
							fallback={(
								<p className="h-4 text-ellipsis whitespace-nowrap font-medium">
									{name}
								</p>
							)}
							when={edit}
						>
							<TextField
								className="text-xl font-bold"
								iconRight={(
									<Button className="px-1.5" onClick={() => setEdit(false)}>
										<CheckIcon height={10} width={10} />
									</Button>
								)}
								onChange={e => updateName(e.target.value)}
								placeholder={name}
							/>
						</Show>
						<p className="h-4 text-xs">
							{mc_loader}
							{' '}
							{mc_version}
						</p>
					</div>

					{/* <LaunchButton cluster={props} iconOnly /> */}
					<Button onClick={handleLaunch} size="icon"><PlayIcon /></Button>
				</div>
			</div>

			<ContextMenu
				isOpen={isOpen}
				setOpen={setOpen}
				triggerRef={ref}
			>
				<ContextMenu.Item className="flex items-center gap-1" onAction={handleLaunch}>
					<span>Launch</span>
					<PlayIcon className="pb-0.5" height={14} width={14} />
				</ContextMenu.Item>
				<ContextMenu.Separator />
				<ContextMenu.Item onAction={() => setEdit(true)}>
					Rename
				</ContextMenu.Item>
				<ContextMenu.Item onAction={launchFilePicker}>
					Change Icon
				</ContextMenu.Item>
				<ContextMenu.Separator />
				<ContextMenu.Item onAction={() => { navigate({ to: '/app/cluster', search: { id } }); }}>
					Properties
				</ContextMenu.Item>
				<ContextMenu.Item onAction={openClusterDir}>
					Open Folder
				</ContextMenu.Item>
				<ContextMenu.Item className="text-red-500">
					Delete
				</ContextMenu.Item>
				{/* <ContextMenu.Item>
					Export
				</ContextMenu.Item> */}
			</ContextMenu>
		</div>
	);
}
