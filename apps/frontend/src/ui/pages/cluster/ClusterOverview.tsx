import { Edit02Icon, ImagePlusIcon, PlayIcon, Save01Icon, Share07Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { Show, createSignal, untrack } from 'solid-js';
import { useBeforeLeave, useNavigate } from '@solidjs/router';
import * as dialog from '@tauri-apps/plugin-dialog';
import ClusterCover from '../../components/game/ClusterCover';
import LoaderIcon from '../../components/game/LoaderIcon';
import Button from '../../components/base/Button';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import type { Cluster } from '~bindings';
import { upperFirst } from '~utils/primitives';
import useClusterContext from '~ui/hooks/useCluster';
import Modal from '~ui/components/overlay/Modal';
import { bridge } from '~imports';
import { tryResult } from '~ui/hooks/useCommand';
import TextField from '~ui/components/base/TextField';

function ClusterOverview() {
	const [cluster, { refetch }] = useClusterContext();
	const [deleteVisible, setDeleteVisible] = createSignal(false);
	const navigate = useNavigate();

	async function deleteCluster() {
		await bridge.commands.removeCluster(cluster()!.uuid);
		navigate('/');
	}

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<Banner cluster={cluster()!} refetch={refetch} showDeletePrompt={() => setDeleteVisible(true)} />
			</ScrollableContainer>

			<Modal.Delete
				visible={deleteVisible}
				setVisible={setDeleteVisible}
				onDelete={deleteCluster}
			/>
		</Sidebar.Page>
	);
}

interface BannerProps {
	cluster: Cluster;
	refetch: () => void;
	showDeletePrompt: () => void;
}

function Banner(props: BannerProps) {
	const [editMode, setEditMode] = createSignal(false);
	const [newName, setNewName] = createSignal('');
	const [newCover, setNewCover] = createSignal('');

	const [modalVisible, setModalVisible] = createSignal(false);

	async function launch() {
		const [_uuid, _pid] = await tryResult(bridge.commands.runCluster, props.cluster.uuid);
	}

	useBeforeLeave((e) => {
		if (editMode() && madeChanges()) {
			e.preventDefault();
			setModalVisible(true);
		}
	});

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

		setNewCover(selected.path);
	}

	function updateName(name: string) {
		if (name.length > 30 || name.length <= 0)
			return;

		setNewName(name);
	}

	async function pushEdits() {
		await bridge.commands.editCluster(
			props.cluster.uuid,
			newName().length > 0 ? newName() : null,
			newCover().length > 0 ? newCover() : props.cluster.meta.icon_url || props.cluster.meta.icon || null,
		);

		props.refetch();
	}

	function madeChanges() {
		const name = untrack(() => newName());
		const cover = untrack(() => newCover());

		const changedName = (name.length > 0 && name !== props.cluster.meta.name);
		const changedCover = (cover.length > 0 && (cover !== props.cluster.meta.icon_url || cover !== props.cluster.meta.icon));

		return changedName || changedCover;
	}

	async function toggleEditMode() {
		const next = !untrack(() => editMode());

		if (next === false && madeChanges()) {
			setModalVisible(true);
			return;
		}

		setEditMode(next);
	}

	function dontSave() {
		setModalVisible(false);
		setEditMode(false);
	}

	function save() {
		pushEdits();
		setModalVisible(false);
		setEditMode(false);
	}

	return (
		<div class="flex flex-row bg-component-bg rounded-xl p-2.5 gap-x-2.5 h-37">
			<div class="rounded-lg overflow-hidden border border-gray-10 relative h-full w-auto aspect-ratio-video">
				<Show when={editMode()}>
					<div
						onClick={launchFilePicker}
						class="bg-black/50 opacity-50 hover:opacity-100 w-full h-full absolute flex justify-center items-center"
					>
						<ImagePlusIcon class="w-12 h-12" />
					</div>
				</Show>

				<ClusterCover override={newCover()} cluster={props.cluster} class="h-full w-full object-cover" />
			</div>

			<div class="flex flex-col flex-1 gap-y-.5 justify-between text-fg-primary">
				<div>
					<Show
						when={editMode()}
						fallback={
							<h2 class="text-2xl">{props.cluster.meta.name}</h2>
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

				<div class={`flex flex-1 flex-col justify-between items-start ${editMode() ? 'opacity-20' : ''}`}>
					<span class="flex flex-row items-center gap-x-1">
						<LoaderIcon loader={props.cluster.meta.loader} class="w-5" />
						<span>{upperFirst(props.cluster.meta.loader || 'unknown')}</span>
						{props.cluster.meta.loader_version && <span>{props.cluster.meta.loader_version.id}</span>}
						<span>{props.cluster.meta.mc_version}</span>
					</span>
					<span class="text-xs text-fg-secondary">
						Played for
						{' '}
						<b>{((props.cluster.meta.overall_played || 0 as unknown as bigint)).toString()}</b>
						{' '}
						hours
					</span>
				</div>

			</div>

			<div class="flex flex-row items-end gap-x-2.5 *:h-8">

				<Button
					buttonStyle="iconSecondary"
					children={<Share07Icon />}
					disabled={editMode()}
				/>

				<Button
					buttonStyle="iconDanger"
					children={<Trash01Icon />}
					onClick={props.showDeletePrompt}
					disabled={editMode()}
				/>

				<Button
					buttonStyle="primary"
					iconLeft={<PlayIcon />}
					children="Launch"
					class="!w-auto"
					onClick={launch}
					disabled={editMode()}
				/>

				<Button.Toggle
					buttonStyle="iconSecondary"
					children={(
						<Show
							when={editMode() === false}
							fallback={<Save01Icon />}
							children={<Edit02Icon />}
						/>
					)}
					checked={editMode}
					onChecked={toggleEditMode}
				/>
			</div>

			<Modal.Simple
				title="Save Changes?"
				visible={modalVisible}
				setVisible={setModalVisible}
				children="Do you want to save your changes?"
				buttons={[
					<Button
						buttonStyle="secondary"
						children="Cancel"
						onClick={() => setModalVisible(false)}
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
		</div>
	);
}

export default ClusterOverview;
