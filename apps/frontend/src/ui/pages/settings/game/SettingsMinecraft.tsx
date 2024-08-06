import { CpuChip01Icon, Database01Icon, EyeIcon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, VariableIcon, XIcon } from '@untitled-theme/icons-solid';
import { type Accessor, type Setter, createSignal, onMount, untrack } from 'solid-js';
import TextField from '~ui/components/base/TextField';
import Toggle from '~ui/components/base/Toggle';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import SettingsRow from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import useSettingsContext from '~ui/hooks/useSettings';
import { asEnvVariables } from '~utils';

// TODO: REFACTOR THIS ENTIRE FILE AND POSSIBLY `apps/frontend/src/ui/pages/cluster/ClusterSettings.tsx` TOO

function SettingsMinecraft() {
	return (
		<Sidebar.Page>
			<h1>Global Minecraft Settings</h1>
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
	isGlobal: Accessor<boolean>;
	resetToFallback: (raw?: any) => void;
};
export function createSetting<T>(initial: T | undefined | null, fallback?: T): CreateSetting<T> {
	const [value, setValue] = createSignal<T>(initial || fallback as T);
	const [raw, setRaw] = createSignal<T>(initial as T);
	const [isGlobal, setIsGlobal] = createSignal(true);

	const checkGlobal = () => {
		const check_one = untrack(() => value()) === undefined || untrack(() => value()) === null;
		const check_two = initial === undefined || initial === null;
		// const check_three = untrack(() => raw()) === undefined || untrack(() => raw()) === null;

		setIsGlobal(check_one || check_two);
	};

	onMount(() => {
		checkGlobal();
	});

	// @ts-expect-error -- aaa
	const set: Setter<T> = (value) => {
		// @ts-expect-error -- aaa
		setRaw(value);
		// @ts-expect-error -- aaa
		setValue(value);

		checkGlobal();
	};

	const resetToFallback = (raw?: T) => {
		if (fallback === undefined || fallback === null)
			throw new Error('No fallback value provided');

		setValue(() => fallback);

		// @ts-expect-error -- aaa
		setRaw(() => (raw || null));

		checkGlobal();
	};

	const getRaw = () => raw();

	return {
		get: value,
		getRaw,
		set,
		isGlobal,
		resetToFallback,
	};
}

function PageSettings() {
	const { settings, saveOnLeave } = useSettingsContext();

	// Game
	const fullscreen = createSetting(settings().force_fullscreen ?? false);
	const resolution = createSetting(settings().resolution);
	const memory = createSetting(settings().memory);

	// Launcher
	const hideOnLaunch = createSetting(settings().hide_on_launch ?? false);

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

	const GameSettings = () => (
		<>
			<SettingsRow.Header>Game</SettingsRow.Header>

			<SettingsRow
				title="Force Fullscreen"
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
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
			<SettingsRow.Header>Launcher</SettingsRow.Header>

			<SettingsRow
				title="Hide On Launch"
				description="Hide the launcher whenever you start a game."
				icon={<EyeIcon />}
			>
				<Toggle
					checked={hideOnLaunch.get}
					onChecked={hideOnLaunch.set}
				/>
			</SettingsRow>
		</>
	);

	const ProcessSettings = () => (
		<>
			<SettingsRow.Header>Process</SettingsRow.Header>

			<SettingsRow
				title="Pre-Launch Command"
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
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
			<SettingsRow.Header>Java</SettingsRow.Header>

			<SettingsRow
				title="Version"
				description="Choose the JRE (Java Runtime Environment) used for the game."
				icon={<CpuChip01Icon />}
			/>

			<SettingsRow
				title="Arguments"
				description="Additional arguments passed to the JVM (Java Virtual Machine)."
				icon={<FilePlus02Icon />}
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

export default SettingsMinecraft;
