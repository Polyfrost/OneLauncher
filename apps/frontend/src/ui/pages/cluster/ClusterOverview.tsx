import { Edit02Icon, FolderIcon, ImagePlusIcon, LinkExternal01Icon, PlayIcon, Save01Icon, Share07Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { type Accessor, type Setter, Show, createSignal, untrack } from 'solid-js';
import { useBeforeLeave, useNavigate } from '@solidjs/router';
import * as dialog from '@tauri-apps/plugin-dialog';
import { open } from '@tauri-apps/plugin-shell';
import ClusterCover from '../../components/game/ClusterCover';
import LoaderIcon from '../../components/game/LoaderIcon';
import Button from '../../components/base/Button';
import ClusterRoot from './ClusterRoot';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import type { Cluster } from '~bindings';
import { secondsToWords, upperFirst } from '~utils/primitives';
import useClusterContext from '~ui/hooks/useCluster';
import Modal from '~ui/components/overlay/Modal';
import { bridge } from '~imports';
import TextField from '~ui/components/base/TextField';
import SettingsRow from '~ui/components/SettingsRow';
import useSettingsContext from '~ui/hooks/useSettings';
import joinPath from '~utils/helpers';

function ClusterOverview() {
	const settings = useSettingsContext();
	const [cluster, { refetch }] = useClusterContext();
	const [deleteVisible, setDeleteVisible] = createSignal(false);
	const [saveVisible, setSaveVisible] = createSignal(false);

	const [editMode, setEditMode] = createSignal(false);
	const [newName, setNewName] = createSignal('');
	const [newCover, setNewCover] = createSignal('');

	const navigate = useNavigate();

	useBeforeLeave((e) => {
		if (editMode() && madeChanges()) {
			e.preventDefault();
			setSaveVisible(true);
		}
	});

	function getPath() {
		const clusterPath = cluster()?.path;
		const configDir = settings.config_dir;

		if (typeof clusterPath !== 'string' || typeof configDir !== 'string')
			return '';

		return joinPath(configDir, 'clusters', clusterPath);
	}

	function openPath() {
		open(getPath());
	}

	async function deleteCluster() {
		await bridge.commands.removeCluster(cluster()!.uuid);
		navigate('/');
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
			setSaveVisible(true);
			return;
		}

		setEditMode(next);
	}

	async function pushEdits() {
		const c = untrack(() => cluster());

		if (!c)
			return;

		await bridge.commands.editCluster(
			c.uuid,
			newName().length > 0 ? newName() : null,
			newCover().length > 0 ? newCover() : c.meta.icon_url || c.meta.icon || null,
		);

		refetch();
	}

	function dontSave() {
		setSaveVisible(false);
		setEditMode(false);
	}

	function save() {
		pushEdits();
		setSaveVisible(false);
		setEditMode(false);
	}

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<Banner
					cluster={cluster()!}
					refetch={refetch}

					saveVisible={saveVisible}
					setSaveVisible={setSaveVisible}

					editMode={editMode}
					newName={newName}
					setNewName={setNewName}
					newCover={newCover}
					setNewCover={setNewCover}
				/>

				<SettingsRow.Header>Folders and Files</SettingsRow.Header>
				<SettingsRow
					title="Cluster Folder"
					description={getPath()}
					icon={<FolderIcon />}
					disabled={editMode()}
					children={(
						<Button
							buttonStyle="primary"
							children="Open"
							iconLeft={<LinkExternal01Icon />}
							onClick={openPath}
							disabled={editMode()}
						/>
					)}
				/>

				<SettingsRow.Header>Cluster Actions</SettingsRow.Header>
				<SettingsRow
					title="Edit Cluster"
					description="Edit the cluster name and cover image."
					icon={<Edit02Icon />}
					children={(
						<Button.Toggle
							buttonStyle={editMode() === false ? 'secondary' : 'primary'}
							iconLeft={(
								<Show
									when={editMode() === false}
									fallback={<Save01Icon />}
									children={<Edit02Icon />}
								/>
							)}
							children={editMode() ? 'Save' : 'Edit'}
							checked={editMode}
							onChecked={toggleEditMode}
						/>
					)}
				/>
				<SettingsRow
					title="Delete Cluster"
					description="Delete this cluster and all its data."
					icon={<Trash01Icon />}
					disabled={editMode()}
					children={(
						<Button
							buttonStyle="danger"
							children="Delete"
							iconLeft={<Trash01Icon />}
							onClick={() => setDeleteVisible(true)}
							disabled={editMode()}
						/>
					)}
				/>
			</ScrollableContainer>

			<Modal.Delete
				visible={deleteVisible}
				setVisible={setDeleteVisible}
				onDelete={deleteCluster}
			/>

			<Modal.Simple
				title="Save Changes?"
				visible={saveVisible}
				setVisible={setSaveVisible}
				children="Do you want to save your changes?"
				buttons={[
					<Button
						buttonStyle="secondary"
						children="Cancel"
						onClick={() => setSaveVisible(false)}
					/>,
					<Button
						buttonStyle="danger"
						children="No"
						onClick={dontSave}
					/>,
					<Button
						buttonStyle="primary"
						children="Yes"
						onClick={save}
					/>,
				]}
			/>
		</Sidebar.Page>
	);
}

interface BannerProps {
	cluster: Cluster;
	saveVisible: Accessor<boolean>;
	setSaveVisible: Setter<boolean>;
	editMode: Accessor<boolean>;
	newName: Accessor<string>;
	setNewName: Setter<string>;
	newCover: Accessor<string>;
	setNewCover: Setter<string>;
	refetch: () => void;
}

function Banner(props: BannerProps) {
	const navigate = useNavigate();

	async function launch() {
		ClusterRoot.launch(navigate, props.cluster.uuid);
	}

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

		props.setNewCover(selected.path);
	}

	function updateName(name: string) {
		if (name.length > 30 || name.length <= 0)
			return;

		props.setNewName(name);
	}

	return (
		<div class="flex flex-row bg-component-bg rounded-xl p-2.5 gap-x-2.5 h-37">
			<div class="rounded-lg overflow-hidden border border-gray-10 relative h-full w-57 min-w-57 aspect-ratio-video">
				<Show when={props.editMode()}>
					<div
						onClick={launchFilePicker}
						class="bg-black/50 opacity-50 hover:opacity-100 w-full h-full absolute flex justify-center items-center"
					>
						<ImagePlusIcon class="w-12 h-12" />
					</div>
				</Show>

				<ClusterCover override={props.newCover()} cluster={props.cluster} class="h-full w-full object-cover" />
			</div>

			<div class="flex flex-col w-full overflow-hidden gap-y-.5 justify-between text-fg-primary">
				<div>
					<Show
						when={props.editMode()}
						fallback={
							<h2 class="text-2xl break-words text-wrap">{props.cluster.meta.name}</h2>
						}
					>
						<TextField
							placeholder={props.cluster.meta.name}
							labelClass="h-10"
							class="text-xl font-bold"
							onChange={e => updateName(e.target.value)}
						/>
					</Show>
				</div>

				<div class="flex flex-1 flex-row">
					<div
						class="flex flex-1 flex-col justify-between items-start"
						classList={{
							'text-fg-primary-disabled': props.editMode(),
						}}
					>
						<span class="flex flex-row items-center gap-x-1">
							<LoaderIcon
								loader={props.cluster.meta.loader}
								class="w-5"
								classList={{
									'opacity-50': props.editMode(),
								}}
							/>
							<span>{upperFirst(props.cluster.meta.loader || 'unknown')}</span>
							{props.cluster.meta.loader_version && <span>{props.cluster.meta.loader_version.id}</span>}
							<span>{props.cluster.meta.mc_version}</span>
						</span>
						<span
							class="text-xs text-fg-secondary"
							classList={{
								'text-fg-secondary-disabled': props.editMode(),
							}}
						>
							Played for
							{' '}
							<b>{secondsToWords((props.cluster.meta.overall_played || 0n))}</b>
							.
						</span>
					</div>

					<div class="flex flex-row items-end gap-x-2.5 *:h-8">

						<Button
							buttonStyle="iconSecondary"
							children={<Share07Icon />}
							disabled={props.editMode()}
						/>

						<Button
							buttonStyle="primary"
							iconLeft={<PlayIcon />}
							children="Launch"
							class="!w-auto"
							onClick={launch}
							disabled={props.editMode()}
						/>
					</div>
				</div>

			</div>
		</div>
	);
}

export default ClusterOverview;
