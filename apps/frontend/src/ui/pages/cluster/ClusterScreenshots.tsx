import { convertFileSrc } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import { LinkExternal01Icon } from '@untitled-theme/icons-solid';
import { join } from 'pathe';
import { For, Show } from 'solid-js';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import useCommand from '~ui/hooks/useCommand';
import useSettingsContext from '~ui/hooks/useSettings';

function ClusterScreenshots() {
	const { settings } = useSettingsContext();
	const [cluster] = useClusterContext();
	const [list] = useCommand(bridge.commands.getScreenshots, cluster()!.uuid!);

	function openFolder() {
		open(join(settings().config_dir || '', 'clusters', cluster()?.path || '', 'screenshots'));
	}

	return (
		<Sidebar.Page>
			<h1>Screenshots</h1>
			<ScrollableContainer>
				<Show
					fallback={<div class="text-gray-400">No screenshots found. Press F2 in game to take a screenshot!</div>}
					when={list() !== undefined && list()!.length > 0}
				>
					<div class="grid grid-cols-[repeat(auto-fill,minmax(350px,1fr))] w-full gap-2 transform-gpu">
						<For each={list()!}>
							{screenshot => (
								<ScreenshotEntry path={screenshot} cluster_path={cluster()?.path || ''} />
							)}
						</For>
					</div>
				</Show>
			</ScrollableContainer>

			<div class="flex flex-row justify-end items-end mt-2">
				<Button
					buttonStyle="primary"
					iconLeft={<LinkExternal01Icon />}
					children="Open Folder"
					onClick={openFolder}
				/>
			</div>
		</Sidebar.Page>
	);
}

export default ClusterScreenshots;

function ScreenshotEntry(props: { path: string; cluster_path: string }) {
	const { settings } = useSettingsContext();

	const dir = () => join(settings().config_dir || '', 'clusters', props.cluster_path, 'screenshots');
	const pathSrc = () => convertFileSrc(join(dir(), props.path));

	function onClick() {
		// TODO: Probably make a cool image viewer or something
		open(dir());
	}

	return (
		<div
			class="bg-component-bg hover:bg-component-bg-hover hover:opacity-80 active:bg-component-bg-pressed p-3 gap-3 rounded-xl flex flex-col items-center"
			onClick={onClick}
		>
			<img src={pathSrc()} alt={props.path} class="aspect-ratio-video w-full rounded-lg" />
		</div>
	);
}
