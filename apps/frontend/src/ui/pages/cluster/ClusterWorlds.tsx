import { open } from '@tauri-apps/plugin-shell';
import { LinkExternal01Icon, Trash01Icon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import WorldIcon from '~ui/components/game/WorldIcon';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand from '~ui/hooks/useCommand';
import useSettings from '~ui/hooks/useSettings';
import { join } from 'pathe';
import { For, Show } from 'solid-js';

function ClusterWorlds() {
	const { settings } = useSettings();
	const [cluster] = useClusterContext();
	const [list] = useCommand(() => bridge.commands.getWorlds(cluster()!.uuid));

	function openFolder() {
		open(join(settings().config_dir || '', 'clusters', cluster()?.path || '', 'saves'));
	}

	return (
		<Sidebar.Page>
			<h1>Worlds</h1>
			<ScrollableContainer>
				<Show
					fallback={<div class="text-border/400">No worlds found.</div>}
					when={list() !== undefined && list()!.length > 0}
				>
					<div class="flex flex-col gap-2">
						<For each={list()!}>
							{world_name => (
								<WorldEntry cluster_path={cluster()?.path || ''} name={world_name} />
							)}
						</For>
					</div>
				</Show>
			</ScrollableContainer>

			<div class="mt-2 flex flex-row items-end justify-end">
				<Button
					buttonStyle="primary"
					children="Open Folder"
					iconLeft={<LinkExternal01Icon />}
					onClick={openFolder}
				/>
			</div>
		</Sidebar.Page>
	);
}

export default ClusterWorlds;

function WorldEntry(props: { name: string; cluster_path: string }) {
	const { settings } = useSettings();

	const dir = () => join(settings().config_dir || '', 'clusters', props.cluster_path, 'saves', props.name);

	const deleteModal = createModal(self => (
		<Modal.Delete
			{...self}
			name={`world '${props.name}'`}
			onDelete={() => {
				// bridge.commands.deleteWorld(props.name);
				self.hide();
			}}
			title={`Delete '${props.name}'`}
		/>
	));

	function onClick() {
		// TODO: World region viewer or whatever??
		open(dir());
	}

	function deleteWorld(e: Event) {
		e.preventDefault();
		e.stopImmediatePropagation();

		deleteModal.show();
	}

	return (
		<div
			class="flex flex-row items-center justify-between gap-3 rounded-xl bg-component-bg p-3 active:bg-component-bg-pressed hover:bg-component-bg-hover"
			onClick={onClick}
		>
			<div class="flex flex-row items-center gap-x-3">
				<WorldIcon class="aspect-ratio-square h-16 w-16" cluster_name={props.cluster_path} world_name={props.name} />
				<div class="flex flex-col gap-y-2">
					<h3>{props.name}</h3>
					<p>Todo</p>
				</div>
			</div>

			<div class="flex flex-row items-center justify-end gap-x-3">
				<Button
					buttonStyle="iconDanger"
					children={<Trash01Icon />}
					onClick={deleteWorld}
				/>
			</div>
		</div>
	);
}
