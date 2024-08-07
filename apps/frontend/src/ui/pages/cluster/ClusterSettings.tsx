import { useBeforeLeave } from '@solidjs/router';
import { type Accessor, Show, splitProps } from 'solid-js';
import { CpuChip01Icon, Database01Icon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, VariableIcon, XIcon } from '@untitled-theme/icons-solid';
import { createSetting } from '../settings/game/SettingsMinecraft';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import type { SettingsRowProps } from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useClusterContext from '~ui/hooks/useCluster';
import BaseSettingsRow from '~ui/components/SettingsRow';
import type { Cluster } from '~bindings';
import useSettingsContext from '~ui/hooks/useSettings';
import Toggle from '~ui/components/base/Toggle';
import TextField from '~ui/components/base/TextField';
import { asEnvVariables } from '~utils';
import { tryResult } from '~ui/hooks/useCommand';
import { bridge } from '~imports';

function ClusterSettings() {
	const [cluster] = useClusterContext();

	return (
		<Sidebar.Page>
			<h1>Overview</h1>
			<ScrollableContainer>
				<Show when={cluster() !== undefined}>
					{PageSettings(() => cluster()!)}
				</Show>
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

function SettingsRow(props: SettingsRowProps & {
	reset: () => any;
	isGlobal: Accessor<boolean>;
}) {
	const [split, rest] = splitProps(props, ['children', 'reset', 'isGlobal']);

	// TODO: Reset button doesn't show up immediately after changing a setting

	return (
		<BaseSettingsRow {...rest}>
			<Show when={split.isGlobal() === false}>
				<div
					class="px-2 py-1 text-xs text-white bg-brand hover:bg-brand-hover active:bg-brand-pressed rounded-md font-italic font-medium"
					onClick={() => split.reset()}
				>
					Reset
				</div>
			</Show>

			{split.children}
		</BaseSettingsRow>
	);
}

function PageSettings(cluster: Accessor<Cluster>) {
	const { settings } = useSettingsContext();

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
	const javaArgs = createSetting(cluster().java?.custom_arguments, settings().custom_java_args);
	const envVars = createSetting(cluster().java?.custom_env_arguments, settings().custom_env_args);

	useBeforeLeave(() => {
		tryResult(bridge.commands.editGameSettings, cluster().uuid, {
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
				custom_arguments: javaArgs.getRaw(),
				custom_env_arguments: envVars.getRaw(),
			},
		});
	});

	const GameSettings = () => (
		<>
			<BaseSettingsRow.Header>Game</BaseSettingsRow.Header>

			<SettingsRow
				title="Force Fullscreen"
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
				isGlobal={fullscreen.isGlobal}
				reset={fullscreen.resetToFallback}
			>
				<Toggle
					checked={fullscreen.get}
					onChecked={fullscreen.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Resolution"
				description="The game window resolution in pixels."
				icon={<LayoutTopIcon />}
				isGlobal={resolution.isGlobal}
				reset={resolution.resetToFallback}
			>
				<div class="grid grid-justify-center grid-items-center gap-2 grid-cols-[70px_16px_70px]">
					<TextField.Number
						class="text-center"
						value={resolution.get()[0]}
						onValidSubmit={(value) => {
							resolution.set([Number.parseInt(value), resolution.get()[1]]);
						}}
					/>
					<XIcon class="w-4 h-4" />
					<TextField.Number
						class="text-center"
						value={resolution.get()[1]}
						onValidSubmit={(value) => {
							resolution.set([resolution.get()[0], Number.parseInt(value)]);
						}}
					/>
				</div>
			</SettingsRow>

			<SettingsRow
				title="Memory"
				description="The amount of memory in megabytes allocated for the game."
				icon={<Database01Icon />}
				isGlobal={memory.isGlobal}
				reset={memory.resetToFallback}
			>
				<div class="flex flex-justify-center items-center gap-x-4">
					<div class="flex flex-row items-center gap-x-2">
						<span>Min:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							value={memory.get().minimum}
							onValidSubmit={(value) => {
								memory.set({ minimum: Number.parseInt(value), maximum: memory.get().maximum });
							}}
						/>
					</div>

					<div class="flex flex-row items-center gap-x-2">
						<span>Max:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							value={memory.get().maximum}
							onValidSubmit={(value) => {
								memory.set({ minimum: memory.get().minimum, maximum: Number.parseInt(value) });
							}}
						/>
					</div>
				</div>
			</SettingsRow>
		</>
	);

	const LauncherSettings = () => (
		<>
			{/* <BaseSettingsRow.Header>Launcher</BaseSettingsRow.Header>

			<SettingsRow
				title="Hide On Launch"
				description="Hide the launcher whenever you start a game."
				icon={<EyeIcon />}
			>
				<Toggle
					checked={hideOnLaunch.get}
					onChecked={hideOnLaunch.set}
				/>
			</SettingsRow> */}
		</>
	);

	const ProcessSettings = () => (
		<>
			<BaseSettingsRow.Header>Process</BaseSettingsRow.Header>

			<SettingsRow
				title="Pre-Launch Command"
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
				isGlobal={preCommand.isGlobal}
				reset={preCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="echo 'Game started'"
					value={preCommand.get()}
					onValidSubmit={preCommand.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Wrapper Command"
				description="Command to run when launching the game."
				icon={<ParagraphWrapIcon />}
				isGlobal={wrapperCommand.isGlobal}
				reset={wrapperCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="gamescope"
					value={wrapperCommand.get()}
					onValidSubmit={wrapperCommand.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Post-Exit Command"
				description="Command to run after exiting the game."
				icon={<FileX02Icon />}
				isGlobal={postCommand.isGlobal}
				reset={postCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="echo 'Game exited'"
					value={postCommand.get()}
					onValidSubmit={postCommand.set}
				/>
			</SettingsRow>
		</>
	);

	const JvmSettings = () => (
		<>
			<BaseSettingsRow.Header>Java</BaseSettingsRow.Header>

			<SettingsRow
				title="Version"
				description="Choose the JRE (Java Runtime Environment) used for the game."
				icon={<CpuChip01Icon />}
				isGlobal={() => true}
				reset={() => {}}
			/>

			<SettingsRow
				title="Arguments"
				description="Additional arguments passed to the JVM (Java Virtual Machine)."
				icon={<FilePlus02Icon />}
				isGlobal={javaArgs.isGlobal}
				reset={javaArgs.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder='"-Xmx3G"'
					value={javaArgs.get().join(' ')}
					onValidSubmit={(value) => {
						javaArgs.set(value.split(' '));
					}}
				/>
			</SettingsRow>

			<SettingsRow
				title="Environment Variables"
				description="Additional environment variables passed to the JVM (Java Virtual Machine)."
				icon={<VariableIcon />}
				isGlobal={envVars.isGlobal}
				reset={envVars.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder='"JAVA_HOME=/path/to/java"'
					value={envVars.get().join(' ')}
					onValidSubmit={(value) => {
						envVars.set(asEnvVariables(value));
					}}
				/>
			</SettingsRow>
		</>
	);

	return (
		<>
			<GameSettings />
			<LauncherSettings />
			<ProcessSettings />
			<JvmSettings />
		</>
	);
}

export default ClusterSettings;
