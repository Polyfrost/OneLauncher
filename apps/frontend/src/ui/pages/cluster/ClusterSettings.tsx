import { useBeforeLeave } from '@solidjs/router';
import { bridge } from '~imports';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import { tryResult } from '~ui/hooks/useCommand';
import useSettings from '~ui/hooks/useSettings';
import { type Accessor, Show } from 'solid-js';
import type { Cluster } from '@onelauncher/client/bindings';
import { createSetting, GameSettings, JvmSettings, LauncherSettings, ProcessSettings } from '../settings/game/SettingsMinecraft';

function ClusterSettings() {
	const [cluster] = useClusterContext();

	return (
		<Sidebar.Page>
			<h1>Game Settings</h1>
			<ScrollableContainer>
				<Show when={cluster() !== undefined}>
					{PageSettings(() => cluster()!)}
				</Show>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

function PageSettings(cluster: Accessor<Cluster>) {
	const { settings } = useSettings();

	// Game
	const fullscreen = createSetting(cluster().force_fullscreen, settings().force_fullscreen ?? false);
	const resolution = createSetting(cluster().resolution, settings().resolution);
	const memory = createSetting(cluster().memory, settings().memory);

	// Launcher

	// Process
	const preCommand = createSetting(cluster().init_hooks?.pre, settings().init_hooks.pre ?? '');
	const wrapperCommand = createSetting(cluster().init_hooks?.wrapper, settings().init_hooks.wrapper ?? '');
	const postCommand = createSetting(cluster().init_hooks?.post, settings().init_hooks.post ?? '');

	// JVM
	const javaVersion = createSetting(cluster().java?.custom_version || null);
	const javaArgs = createSetting(cluster().java?.custom_arguments, settings().custom_java_args);
	const envVars = createSetting(cluster().java?.custom_env_arguments, settings().custom_env_args);

	useBeforeLeave(() => {
		tryResult(() => bridge.commands.editGameSettings(cluster().uuid, {
			...cluster(),

			// Game
			force_fullscreen: fullscreen.getRaw(),
			resolution: resolution.getRaw(),
			memory: memory.getRaw(),

			// Process
			init_hooks: {
				pre: preCommand.getRaw(),
				wrapper: wrapperCommand.getRaw(),
				post: postCommand.getRaw(),
			},

			// JVM
			java: {
				custom_version: javaVersion.getRaw(),
				custom_arguments: javaArgs.getRaw(),
				custom_env_arguments: envVars.getRaw(),
			},
		}));
	});

	return (
		<>
			<GameSettings
				{...{
					fullscreen,
					memory,
					resolution,
				}}
			/>

			<LauncherSettings
				{...{
					hideOnLaunch: undefined,
					allowParallelClusters: undefined,
				}}
			/>

			<ProcessSettings
				{...{
					preCommand,
					wrapperCommand,
					postCommand,
				}}
			/>

			<JvmSettings
				{...{
					javaVersion,
					javaVersions: undefined,
					javaArgs,
					envVars,
				}}
			/>
		</>
	);
}

export default ClusterSettings;
