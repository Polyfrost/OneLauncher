import type { Cluster } from '@onelauncher/client/bindings';
import { useNavigate } from '@solidjs/router';
import * as dialog from '@tauri-apps/plugin-dialog';
import { open } from '@tauri-apps/plugin-shell';
import { Edit02Icon, FolderIcon, ImagePlusIcon, LinkExternal01Icon, Save01Icon, Share07Icon, Tool02Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import TextField from '~ui/components/base/TextField';
import LaunchButton from '~ui/components/LaunchButton';
import Modal, { createModal, type ModalProps } from '~ui/components/overlay/Modal';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import usePreventLeave from '~ui/hooks/usePreventLeave';
import useSettings from '~ui/hooks/useSettings';
import { formatAsDuration, upperFirst } from '~utils';
import { join } from 'pathe';
import { type Accessor, createSignal, type Setter, Show, untrack } from 'solid-js';
import Button from '../../components/base/Button';
import ClusterCover from '../../components/game/ClusterCover';
import LoaderIcon from '../../components/game/LoaderIcon';

function ClusterOverview() {
	const { settings } = useSettings();
	const [cluster, { refetch }] = useClusterContext();

	const [editMode, setEditMode] = createSignal(false);
	const [newName, setNewName] = createSignal('');
	const [newCover, setNewCover] = createSignal('');

	const navigate = useNavigate();

	const saveModal = createModal(props => (
		<SaveModal
			{...props}
			dontSave={dontSave}
			save={save}
		/>
	));

	const deleteModal = createModal(props => (
		<Modal.Delete
			{...props}
			name={cluster() ? `'${cluster()?.meta.name}'` : 'The Cluster'}
			onDelete={deleteCluster}
			title="Confirm Cluster Deletion"
		/>
	));

	function getPath() {
		const clusterPath = cluster()?.path;
		const configDir = settings().config_dir;

		if (typeof clusterPath !== 'string' || typeof configDir !== 'string')
			return '';

		return join(configDir, 'clusters', clusterPath);
	}

	function openPath() {
		open(getPath());
	}

	async function deleteCluster() {
		await bridge.commands.removeCluster(cluster()!.uuid);
		navigate('/');
	}

	async function repairCluster() {
		await bridge.commands.repairCluster(cluster()!.uuid);
	}

	function madeChanges() {
		const name = untrack(() => newName());
		const cover = untrack(() => newCover());
		const c = untrack(() => cluster());

		if (!c)
			return false;

		const changedName = (name.length > 0 && name !== c.meta.name);
		const changedCover = (cover.length > 0 && (cover !== c.meta.icon_url || cover !== c.meta.icon));

		return changedName || changedCover;
	}

	function toggleEditMode() {
		const next = !untrack(() => editMode());

		if (next === false && madeChanges()) {
			saveModal.show();
			return;
		}

		setEditMode(next);
	}

	async function pushEdits() {
		const c = untrack(() => cluster());

		if (!c)
			return;

		await bridge.commands.editClusterMeta(
			c.uuid,
			newName().length > 0 ? newName() : null,
			newCover().length > 0 ? newCover() : c.meta.icon_url || c.meta.icon || null,
		);

		refetch();
	}

	const preventLeave = usePreventLeave((ctx) => {
		if (editMode() && madeChanges()) {
			ctx.preventNavigation();
			saveModal.show();
		}
	});

	async function dontSave() {
		saveModal.hide();
		setEditMode(false);

		if (preventLeave.triedNavigating())
			preventLeave.continue();
	}

	async function save() {
		await pushEdits();
		saveModal.hide();
		setEditMode(false);

		if (preventLeave.triedNavigating())
			preventLeave.continue();
	}

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<Banner
					cluster={cluster()!}
					editMode={editMode}

					newCover={newCover}
					newName={newName}
					refetch={refetch}
					setNewCover={setNewCover}
					setNewName={setNewName}
				/>

				<SettingsRow.Header>Folders and Files</SettingsRow.Header>
				<SettingsRow
					children={(
						<Button
							buttonStyle="primary"
							children="Open"
							disabled={editMode()}
							iconLeft={<LinkExternal01Icon />}
							onClick={openPath}
						/>
					)}
					description={getPath()}
					disabled={editMode()}
					icon={<FolderIcon />}
					title="Cluster Folder"
				/>

				<SettingsRow.Header>Cluster Actions</SettingsRow.Header>
				<SettingsRow
					children={(
						<Button.Toggle
							buttonStyle={editMode() === false ? 'secondary' : 'primary'}
							checked={editMode}
							children={editMode() ? 'Save' : 'Edit'}
							iconLeft={(
								<Show
									children={<Edit02Icon />}
									fallback={<Save01Icon />}
									when={editMode() === false}
								/>
							)}
							onChecked={toggleEditMode}
						/>
					)}
					description="Edit the cluster name and cover image."
					icon={<Edit02Icon />}
					title="Edit Cluster"
				/>
				<SettingsRow
					children={(
						<Button
							buttonStyle="danger"
							children="Delete"
							disabled={editMode()}
							iconLeft={<Trash01Icon />}
							onClick={() => deleteModal.show()}
						/>
					)}
					description="Delete this cluster and all its data."
					disabled={editMode()}
					icon={<Trash01Icon />}
					title="Delete Cluster"
				/>
				<SettingsRow
					children={(
						<Button
							buttonStyle="secondary"
							children="Repair"
							iconLeft={<Tool02Icon />}
							onClick={repairCluster}
						/>
					)}
					description="Verifies whether all assets, libraries and natives were properly installed."
					icon={<Tool02Icon />}
					title="Verify Cluster"
				/>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface SaveModalProps extends ModalProps {
	save: () => void;
	dontSave: () => void;
}

function SaveModal(p: SaveModalProps) {
	const [modalProps, props] = Modal.SplitProps(p);

	return (
		<Modal.Simple
			{...modalProps}
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Cancel"
					onClick={() => modalProps.hide()}
				/>,
				<Button
					buttonStyle="danger"
					children="No"
					onClick={props.dontSave}
				/>,
				<Button
					buttonStyle="primary"
					children="Yes"
					onClick={props.save}
				/>,
			]}
			children="Do you want to save your changes?"
			title="Save Changes?"
		/>
	);
}

interface BannerProps {
	cluster: Cluster;
	editMode: Accessor<boolean>;
	newName: Accessor<string>;
	setNewName: Setter<string>;
	newCover: Accessor<string>;
	setNewCover: Setter<string>;
	refetch: () => void;
}

function Banner(props: BannerProps) {
	async function launchFilePicker() {
		const selected = await dialog.open({
			multiple: false,
			directory: false,
			filters: [{
				name: 'Image',
				extensions: ['png', 'jpg', 'jpeg', 'webp'],
			}],
		});

		if (selected === null)
			return;

		props.setNewCover(selected);
	}

	function updateName(name: string) {
		if (name.length > 30 || name.length <= 0)
			return;

		props.setNewName(name);
	}

	return (
		<div class="h-37 flex flex-row gap-x-2.5 rounded-xl bg-page-elevated p-2.5">
			<div class="relative aspect-ratio-video h-full min-w-57 w-57 overflow-hidden border border-border/10 rounded-lg">
				<Show when={props.editMode()}>
					<div
						class="absolute z-1 h-full w-full flex items-center justify-center bg-black/50 opacity-50 hover:opacity-100"
						onClick={launchFilePicker}
					>
						<ImagePlusIcon class="h-12 w-12" />
					</div>
				</Show>

				<ClusterCover class="h-full w-full object-cover" cluster={props.cluster} override={props.newCover()} />
			</div>

			<div class="w-full flex flex-col justify-between gap-y-.5 overflow-hidden text-fg-primary">
				<div>
					<Show
						fallback={
							<h2 class="break-words text-wrap text-2xl">{props.cluster.meta.name}</h2>
						}
						when={props.editMode()}
					>
						<TextField
							class="text-xl font-bold"
							labelClass="h-10"
							onChange={e => updateName(e.target.value)}
							placeholder={props.cluster.meta.name}
						/>
					</Show>
				</div>

				<div class="flex flex-1 flex-row">
					<div
						class="flex flex-1 flex-col items-start justify-between"
						classList={{
							'text-fg-primary-disabled': props.editMode(),
						}}
					>
						<span class="flex flex-row items-center gap-x-1">
							<LoaderIcon
								class="w-5"
								classList={{
									'opacity-50': props.editMode(),
								}}
								loader={props.cluster.meta.loader}
							/>
							<span>{props.cluster.meta.mc_version}</span>
							<span>{upperFirst(props.cluster.meta.loader || 'unknown')}</span>
							{props.cluster.meta.loader_version && <span>{props.cluster.meta.loader_version.id}</span>}
						</span>
						<span
							class="text-xs text-fg-secondary"
							classList={{
								'text-fg-secondary-disabled': props.editMode(),
							}}
						>
							Played for
							{' '}
							<b>{formatAsDuration((props.cluster.meta.overall_played || 0))}</b>
							.
						</span>
					</div>

					<div class="flex flex-row items-end gap-x-2.5 *:h-8">
						<Button
							buttonStyle="iconSecondary"
							children={<Share07Icon />}
							// disabled={props.editMode()}
							disabled={true}
						/>

						<LaunchButton cluster={props.cluster} />
					</div>
				</div>

			</div>
		</div>
	);
}

export default ClusterOverview;
