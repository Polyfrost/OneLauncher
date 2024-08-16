import { ActivityIcon, CpuChip01Icon, Database01Icon, EyeIcon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, VariableIcon, XIcon } from '@untitled-theme/icons-solid';
import { type Accessor, type Setter, Show, createSignal, onMount, splitProps, untrack } from 'solid-js';
import type { Memory, Resolution } from '~bindings';
import TextField from '~ui/components/base/TextField';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import BaseSettingsRow, { type SettingsRowProps } from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';
import { asEnvVariables } from '~utils';

function SettingsMinecraft() {
	return (
		<Sidebar.Page>
			<h1>Global Game Settings</h1>
			<ScrollableContainer>
				<PageSettings />
			</ScrollableContainer>
		</Sidebar.Page>
	);
}

interface CreateSetting<T> {
	get: Accessor<T>;
	getRaw: () => T;
	set: Setter<T>;
	isGlobal: Accessor<boolean | null>;
	resetToFallback: (raw?: any) => void;
};

export function createSetting<T>(initial: T | undefined | null, fallback?: T): CreateSetting<T> {
	const [value, setValue] = createSignal<T>((initial === undefined || initial === null) ? fallback as T : initial);
	const [raw, setRaw] = createSignal<T>(initial as T);
	const [isGlobal, setIsGlobal] = createSignal<boolean | null>(true);

	const checkGlobal = () => {
		if (fallback === undefined || fallback === null) {
			setIsGlobal(null);
			return;
		}

		const check_one = untrack(() => raw()) === undefined || untrack(() => raw()) === null;

		setIsGlobal(check_one);
	};

	onMount(() => {
		checkGlobal();
	});

	// @ts-expect-error -- Can't be bothered to fix this type issue
	const set: Setter<T> = (value) => {
		setRaw(value!);
		setValue(value!);

		checkGlobal();
	};

	const resetToFallback = (raw?: T) => {
		if (fallback === undefined || fallback === null)
			throw new Error('No fallback value provided');

		setValue(() => fallback);

		setRaw(() => (raw || null)!);

		checkGlobal();
	};

	return {
		get: value,
		getRaw: raw,
		set,
		isGlobal,
		resetToFallback,
	};
}

function SettingsRow(props: SettingsRowProps & {
	reset: () => any;
	isGlobal: Accessor<boolean | null>;
}) {
	const [split, rest] = splitProps(props, ['children', 'reset', 'isGlobal']);

	return (
		<BaseSettingsRow {...rest}>
			<Show when={split.isGlobal() === false}>
				<div
					class="rounded-md bg-brand px-2 py-1 text-xs text-white font-medium font-italic active:bg-brand-pressed hover:bg-brand-hover"
					onClick={() => split.reset()}
				>
					Reset
				</div>
			</Show>

			{split.children}
		</BaseSettingsRow>
	);
}

export function GameSettings(props: {
	fullscreen: CreateSetting<boolean>;
	resolution: CreateSetting<Resolution>;
	memory: CreateSetting<Memory>;
}) {
	return (
		<>
			<BaseSettingsRow.Header>Game</BaseSettingsRow.Header>

			<SettingsRow
				title="Force Fullscreen"
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
				isGlobal={props.fullscreen.isGlobal}
				reset={props.fullscreen.resetToFallback}
			>
				<Toggle
					checked={props.fullscreen.get}
					onChecked={props.fullscreen.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Resolution"
				description="The game window resolution in pixels."
				icon={<LayoutTopIcon />}
				isGlobal={props.resolution.isGlobal}
				reset={props.resolution.resetToFallback}
			>
				<div class="grid grid-cols-[70px_16px_70px] gap-2 grid-justify-center grid-items-center">
					<TextField.Number
						class="text-center"
						value={props.resolution.get()[0]}
						onValidSubmit={(value) => {
							props.resolution.set([Number.parseInt(value), props.resolution.get()[1]]);
						}}
					/>
					<XIcon class="h-4 w-4" />
					<TextField.Number
						class="text-center"
						value={props.resolution.get()[1]}
						onValidSubmit={(value) => {
							props.resolution.set([props.resolution.get()[0], Number.parseInt(value)]);
						}}
					/>
				</div>
			</SettingsRow>

			<SettingsRow
				title="Memory"
				description="The amount of memory in megabytes allocated for the game."
				icon={<Database01Icon />}
				isGlobal={props.memory.isGlobal}
				reset={props.memory.resetToFallback}
			>
				<div class="flex items-center gap-x-4 flex-justify-center">
					<div class="flex flex-row items-center gap-x-2">
						<span>Min:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							value={props.memory.get().minimum}
							min={1}
							max={props.memory.get().maximum}
							onValidSubmit={(value) => {
								props.memory.set({ minimum: Number.parseInt(value), maximum: props.memory.get().maximum });
							}}
						/>
					</div>

					<div class="flex flex-row items-center gap-x-2">
						<span>Max:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							value={props.memory.get().maximum}
							min={props.memory.get().minimum}
							max={Number.MAX_SAFE_INTEGER}
							onValidSubmit={(value) => {
								props.memory.set({ minimum: props.memory.get().minimum, maximum: Number.parseInt(value) });
							}}
						/>
					</div>
				</div>
			</SettingsRow>
		</>
	);
}

export function LauncherSettings(props: {
	hideOnLaunch: CreateSetting<boolean> | undefined;
	allowParallelClusters: CreateSetting<boolean> | undefined;
}) {
	const shouldShowHeader = () => Object.values(props).some(setting => setting !== undefined);

	return (
		<>
			<Show when={shouldShowHeader()}>
				<BaseSettingsRow.Header>Launcher</BaseSettingsRow.Header>
			</Show>

			<Show when={props.hideOnLaunch !== undefined}>
				<SettingsRow
					title="Hide On Launch"
					description="Hide the launcher whenever you start a game."
					icon={<EyeIcon />}
					isGlobal={props.hideOnLaunch!.isGlobal}
					reset={props.hideOnLaunch!.resetToFallback}
				>
					<Toggle
						checked={props.hideOnLaunch!.get}
						onChecked={props.hideOnLaunch!.set}
					/>
				</SettingsRow>
			</Show>

			<Show when={props.allowParallelClusters !== undefined}>
				<SettingsRow
					title="Allow Parallel Clusters"
					description="Allow running the same cluster with the same account."
					icon={<ActivityIcon />}
					isGlobal={props.allowParallelClusters!.isGlobal}
					reset={props.allowParallelClusters!.resetToFallback}
				>
					<Toggle
						checked={props.allowParallelClusters!.get}
						onChecked={props.allowParallelClusters!.set}
					/>
				</SettingsRow>
			</Show>
		</>
	);
}

export function ProcessSettings(props: {
	preCommand: CreateSetting<string>;
	wrapperCommand: CreateSetting<string>;
	postCommand: CreateSetting<string>;
}) {
	return (
		<>
			<BaseSettingsRow.Header>Process</BaseSettingsRow.Header>

			<SettingsRow
				title="Pre-Launch Command"
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
				isGlobal={props.preCommand.isGlobal}
				reset={props.preCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="echo 'Game started'"
					value={props.preCommand.get()}
					onValidSubmit={props.preCommand.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Wrapper Command"
				description="Command to run when launching the game."
				icon={<ParagraphWrapIcon />}
				isGlobal={props.wrapperCommand.isGlobal}
				reset={props.wrapperCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="gamescope"
					value={props.wrapperCommand.get()}
					onValidSubmit={props.wrapperCommand.set}
				/>
			</SettingsRow>

			<SettingsRow
				title="Post-Exit Command"
				description="Command to run after exiting the game."
				icon={<FileX02Icon />}
				isGlobal={props.postCommand.isGlobal}
				reset={props.postCommand.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder="echo 'Game exited'"
					value={props.postCommand.get()}
					onValidSubmit={props.postCommand.set}
				/>
			</SettingsRow>
		</>
	);
}

export function JvmSettings(props: {
	javaArgs: CreateSetting<string[]>;
	envVars: CreateSetting<[string, string][]>;
}) {
	return (
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
				isGlobal={props.javaArgs.isGlobal}
				reset={props.javaArgs.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder='"-Xmx3G"'
					value={props.javaArgs.get().join(' ')}
					onValidSubmit={(value) => {
						props.javaArgs.set(value.split(' '));
					}}
				/>
			</SettingsRow>

			<SettingsRow
				title="Environment Variables"
				description="Additional environment variables passed to the JVM (Java Virtual Machine)."
				icon={<VariableIcon />}
				isGlobal={props.envVars.isGlobal}
				reset={props.envVars.resetToFallback}
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					placeholder='"JAVA_HOME=/path/to/java"'
					value={props.envVars.get().join(' ')}
					onValidSubmit={(value) => {
						props.envVars.set(asEnvVariables(value));
					}}
				/>
			</SettingsRow>
		</>
	);
}

function PageSettings() {
	const { settings, saveOnLeave } = useSettingsContext();

	// Game
	const fullscreen = createSetting(settings().force_fullscreen ?? false);
	const resolution = createSetting(settings().resolution);
	const memory = createSetting(settings().memory);

	// Launcher
	const hideOnLaunch = createSetting(settings().hide_on_launch ?? false);
	const allowParallelClusters = createSetting(settings().allow_parallel_running_clusters ?? false);

	// Process
	const preCommand = createSetting(settings().init_hooks.pre ?? '');
	const wrapperCommand = createSetting(settings().init_hooks.wrapper ?? '');
	const postCommand = createSetting(settings().init_hooks.post ?? '');

	// JVM
	const javaArgs = createSetting(settings().custom_java_args);
	const envVars = createSetting(settings().custom_env_args);

	saveOnLeave(() => ({
		// Game
		force_fullscreen: fullscreen.get(),
		resolution: resolution.get(),
		memory: memory.get(),

		// Launcher
		hide_on_launch: hideOnLaunch.get(),
		allow_parallel_running_clusters: allowParallelClusters.get(),

		// Process
		init_hooks: {
			pre: preCommand.get(),
			wrapper: wrapperCommand.get(),
			post: postCommand.get(),
		},

		// JVM
		custom_java_args: javaArgs.get(),
		custom_env_args: envVars.get(),
	}));

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
					hideOnLaunch,
					allowParallelClusters,
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
					javaArgs,
					envVars,
				}}
			/>
		</>
	);
}

export default SettingsMinecraft;
