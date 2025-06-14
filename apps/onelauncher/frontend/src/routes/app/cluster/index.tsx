import type { Model } from '@/bindings.gen';
import type { Dispatch, SetStateAction } from 'react';
import LoaderIcon from '@/components/launcher/LoaderIcon';
import ScrollableContainer from '@/components/ScrollableContainer';
import SettingsRow from '@/components/SettingsRow';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Show, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Edit02Icon, FolderIcon, ImagePlusIcon, Share07Icon, Tool02Icon, Trash01Icon } from '@untitled-theme/icons-react';
import { useState } from 'react';
import { twMerge } from 'tailwind-merge';
import Sidebar from '../settings/route';

export const Route = createFileRoute('/app/cluster/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { id } = Route.useSearch();
	const [newCover, setNewCover] = useState<string>('');
	const [newName, setNewName] = useState<string>('');
	const [edit, setEdit] = useState<boolean>(false);

	// dumbass fix ik
	const cluster = useCommand('getClusterById', () => bindings.core.getClusterById(Number(id.toString()) as unknown as bigint));

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<div className="h-full">
					<Banner
						cluster={cluster.data}
						editMode={edit}
						newCover={newCover}
						newName={newName}
						refetch={cluster.refetch}
						setNewCover={setNewCover}
						setNewName={setNewName}
					/>

					<SettingsRow.Header>Folders and Files</SettingsRow.Header>
					<SettingsRow
						children={(
							<Button
								children="Open"
								color="primary"
								isDisabled={false}
							/>
						)}
						description="burada path var işte yersen"
						disabled={false}
						icon={<FolderIcon />}
						title="Cluster Folder"
					/>

					<SettingsRow.Header>Cluster Actions</SettingsRow.Header>
					<SettingsRow
						children={(
							<Button color="secondary" onClick={() => setEdit(!edit)}>{edit ? 'Save' : 'Edit'}</Button>
						)}
						description="Edit the cluster name and cover image."
						icon={<Edit02Icon />}
						title="Edit Cluster"
					/>
					<SettingsRow
						children={(
							<Button
								children="Delete"
								color="danger"
								isDisabled={false}
							/>
						)}
						description="Delete this cluster and all its data."
						disabled={false}
						icon={<Trash01Icon />}
						title="Delete Cluster"
					/>
					<SettingsRow
						children={(
							<Button
								children="Repair"
								color="secondary"
							/>
						)}
						description="Verifies whether all assets, libraries and natives were properly installed."
						icon={<Tool02Icon />}
						title="Verify Cluster"
					/>
				</div>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface BannerProps {
	cluster: Model | null | undefined;
	editMode: boolean;
	newName: string;
	setNewName: Dispatch<SetStateAction<string>>;
	newCover: string;
	setNewCover: Dispatch<SetStateAction<string>>;
	refetch: () => void;
}

function Banner({
	cluster,
	editMode,
	setNewName,
}: BannerProps) {
	// async function launchFilePicker() {
	// 	const selected = await dialog.open({
	// 		multiple: false,
	// 		directory: false,
	// 		filters: [{
	// 			name: 'Image',
	// 			extensions: ['png', 'jpg', 'jpeg', 'webp'],
	// 		}],
	// 	});

	// 	if (selected === null)
	// 		return;

	// 	props.setNewCover(selected);
	// }
	const launch = useCommand('launchCluster', () => bindings.core.launchCluster(cluster?.id as bigint, null), {
		enabled: false,
		subscribed: false,
	});

	const handleLaunch = () => {
		launch.refetch();

		if (launch.error)
			console.error(launch.error.message);
	};

	function updateName(name: string) {
		if (name.length > 30 || name.length <= 0)
			return;

		setNewName(name);
	}

	return (
		<div className="h-37 flex flex-row gap-x-2.5 rounded-xl bg-page-elevated p-2.5">
			<div className="relative aspect-ratio-video h-full min-w-57 w-57 overflow-hidden border border-component-bg/10 rounded-lg">
				<Show when={editMode}>
					<div
						className="absolute z-1 h-full w-full flex items-center justify-center bg-black/50 opacity-50 hover:opacity-100"
						// onClick={launchFilePicker}
					>
						<ImagePlusIcon className="h-12 w-12" />
					</div>
				</Show>

				{/* <ClusterCover class="h-full w-full object-cover" cluster={props.cluster} override={props.newCover()} /> */}
				<img src="https://github.com/emirsassan.png" />
			</div>

			<div className="w-full flex flex-col justify-between gap-y-.5 overflow-hidden text-fg-primary">
				<div>
					<Show
						fallback={
							<h2 className="break-words text-wrap text-2xl">{cluster?.name}</h2>
						}
						when={editMode}
					>
						<TextField
							className="text-xl font-bold"
							onChange={e => updateName(e.target.value)}
							placeholder={cluster?.name}
						/>
					</Show>
				</div>

				<div className="flex flex-1 flex-row">
					<div
						className={twMerge(`flex flex-1 flex-col items-start justify-between`, editMode && 'text-fg-primary-disabled')}
					>
						<span className="flex flex-row items-center gap-x-1">
							<LoaderIcon
								className="w-5"
								loader={cluster?.mc_loader}
							/>
							<span>{cluster?.mc_version}</span>
							<span>{cluster?.mc_loader || 'unknown'}</span>
							{cluster?.mc_loader_version && <span>{cluster.mc_loader_version}</span>}
						</span>
						<span
							className={twMerge(`text-xs text-fg-secondary`, editMode && 'text-fg-secondary-disabled')}
						>
							Played for
							{' '}
							<b>{cluster?.overall_played || 0}</b>
							.
						</span>
					</div>

					<div className="flex flex-row items-end gap-x-2.5 *:h-8">
						<Button children="Launch" onClick={handleLaunch} />

						<Button
							children={<Share07Icon />}
							color="secondary"
							// disabled={props.editMode()}
							isDisabled
						/>
					</div>
				</div>

			</div>
		</div>
	);
}
