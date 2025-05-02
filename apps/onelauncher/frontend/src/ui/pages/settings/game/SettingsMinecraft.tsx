import type { JavaVersion, JavaVersions, JavaZuluPackage, Memory, Resolution } from '@onelauncher/client/bindings';
import { ActivityIcon, CpuChip01Icon, Database01Icon, Download01Icon, EyeIcon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, VariableIcon, XIcon } from '@untitled-theme/icons-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import Toggle from '~ui/components/base/Toggle';
import Modal, { createModal, type ModalProps } from '~ui/components/overlay/Modal';
import ScrollableContainer from '~ui/components/ScrollableContainer';
import BaseSettingsRow, { type SettingsRowProps } from '~ui/components/SettingsRow';
import Sidebar from '~ui/components/Sidebar';
import { tryResult } from '~ui/hooks/useCommand';
import useSettings from '~ui/hooks/useSettings';
import { asEnvVariables } from '~utils';
import { type Accessor, createMemo, createResource, createSignal, For, onMount, type Setter, Show, splitProps, untrack } from 'solid-js';

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
	resetToFallback: (raw?: T) => void;
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

	const set: Setter<T> = (newValue?: any) => {
		setRaw(newValue!);
		setValue(newValue!);
		checkGlobal();

		return value as any;
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
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
				isGlobal={props.fullscreen.isGlobal}
				reset={props.fullscreen.resetToFallback}
				title="Force Fullscreen"
			>
				<Toggle
					checked={props.fullscreen.get}
					onChecked={props.fullscreen.set}
				/>
			</SettingsRow>

			<SettingsRow
				description="The game window resolution in pixels."
				icon={<LayoutTopIcon />}
				isGlobal={props.resolution.isGlobal}
				reset={props.resolution.resetToFallback}
				title="Resolution"
			>
				<div class="grid grid-cols-[70px_16px_70px] gap-2 grid-justify-center grid-items-center">
					<TextField.Number
						class="text-center"
						onValidSubmit={(value) => {
							props.resolution.set([Number.parseInt(value), props.resolution.get()[1]]);
						}}
						value={props.resolution.get()[0]}
					/>
					<XIcon class="h-4 w-4" />
					<TextField.Number
						class="text-center"
						onValidSubmit={(value) => {
							props.resolution.set([props.resolution.get()[0], Number.parseInt(value)]);
						}}
						value={props.resolution.get()[1]}
					/>
				</div>
			</SettingsRow>

			{/* TODO: make this a memory slider */}
			<SettingsRow
				description="The amount of memory in megabytes allocated for the game."
				icon={<Database01Icon />}
				isGlobal={props.memory.isGlobal}
				reset={props.memory.resetToFallback}
				title="Memory"
			>
				<div class="flex items-center gap-x-4 flex-justify-center">
					<div class="flex flex-row items-center gap-x-2">
						<span>Min:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							max={props.memory.get().maximum}
							min={1}
							onValidSubmit={(value) => {
								props.memory.set({ minimum: Number.parseInt(value), maximum: props.memory.get().maximum });
							}}
							value={props.memory.get().minimum}
						/>
					</div>

					<div class="flex flex-row items-center gap-x-2">
						<span>Max:</span>
						<TextField.Number
							class="text-center"
							labelClass="w-[70px]!"
							max={Number.MAX_SAFE_INTEGER}
							min={props.memory.get().minimum}
							onValidSubmit={(value) => {
								props.memory.set({ minimum: props.memory.get().minimum, maximum: Number.parseInt(value) });
							}}
							value={props.memory.get().maximum}
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
					description="Hide the launcher whenever you start a game."
					icon={<EyeIcon />}
					isGlobal={props.hideOnLaunch!.isGlobal}
					reset={props.hideOnLaunch!.resetToFallback}
					title="Hide On Launch"
				>
					<Toggle
						checked={props.hideOnLaunch!.get}
						onChecked={props.hideOnLaunch!.set}
					/>
				</SettingsRow>
			</Show>

			<Show when={props.allowParallelClusters !== undefined}>
				<SettingsRow
					description="Allow running the same cluster with the same account."
					icon={<ActivityIcon />}
					isGlobal={props.allowParallelClusters!.isGlobal}
					reset={props.allowParallelClusters!.resetToFallback}
					title="Allow Parallel Clusters"
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
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
				isGlobal={props.preCommand.isGlobal}
				reset={props.preCommand.resetToFallback}
				title="Pre-Launch Command"
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					onValidSubmit={props.preCommand.set}
					placeholder="echo 'Game started'"
					value={props.preCommand.get()}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run when launching the game."
				icon={<ParagraphWrapIcon />}
				isGlobal={props.wrapperCommand.isGlobal}
				reset={props.wrapperCommand.resetToFallback}
				title="Wrapper Command"
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					onValidSubmit={props.wrapperCommand.set}
					placeholder="gamescope"
					value={props.wrapperCommand.get()}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run after exiting the game."
				icon={<FileX02Icon />}
				isGlobal={props.postCommand.isGlobal}
				reset={props.postCommand.resetToFallback}
				title="Post-Exit Command"
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					onValidSubmit={props.postCommand.set}
					placeholder="echo 'Game exited'"
					value={props.postCommand.get()}
				/>
			</SettingsRow>
		</>
	);
}

export function JvmSettings(props: {
	javaArgs: CreateSetting<string[]>;
	envVars: CreateSetting<[string, string][]>;
} & ({
	clusterId: string;
	javaVersion: CreateSetting<JavaVersion | null>;
	javaVersions: CreateSetting<JavaVersions>;
} | {
	javaVersion: undefined;
	javaVersions: CreateSetting<JavaVersions>;
})) {
	const modal = createModal((controller) => {
		if (props.javaVersion)
			return (
				<ClusterJavaVersionModal
					{...controller}
					clusterId={props.clusterId}
					javaVersion={props.javaVersion}
					javaVersions={props.javaVersions}
				/>
			);
		else return (
			<GlobalJavaVersionModal
				{...controller}
				javaVersions={props.javaVersions}
			/>
		);
	});

	return (
		<>
			<BaseSettingsRow.Header>Java</BaseSettingsRow.Header>

			<SettingsRow
				description="Choose the JRE (Java Runtime Environment) used for the game."
				icon={<CpuChip01Icon />}
				isGlobal={() => true}
				reset={() => {}}
				title="Version"
			>
				<Button
					children="Configure"
					iconLeft={<Database01Icon />}
					onClick={modal.show}
				/>
			</SettingsRow>

			<SettingsRow
				description="Additional arguments passed to the JVM (Java Virtual Machine)."
				icon={<FilePlus02Icon />}
				isGlobal={props.javaArgs.isGlobal}
				reset={props.javaArgs.resetToFallback}
				title="Arguments"
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					onValidSubmit={(value) => {
						props.javaArgs.set(value.split(' '));
					}}
					placeholder='"-Xmx3G"'
					value={props.javaArgs.get().join(' ')}
				/>
			</SettingsRow>

			<SettingsRow
				description="Additional environment variables passed to the JVM (Java Virtual Machine)."
				icon={<VariableIcon />}
				isGlobal={props.envVars.isGlobal}
				reset={props.envVars.resetToFallback}
				title="Environment Variables"
			>
				<TextField
					labelClass="w-full! min-w-[260px]!"
					onValidSubmit={(value) => {
						props.envVars.set(asEnvVariables(value));
					}}
					placeholder='"JAVA_HOME=/path/to/java"'
					value={props.envVars.get().join(' ')}
				/>
			</SettingsRow>
		</>
	);
}

function PageSettings() {
	const { settings, saveOnLeave } = useSettings();

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
	const javaVersions = createSetting(settings().java_versions);
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
		java_versions: javaVersions.get(),
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
					javaVersion: undefined,
					javaVersions,
					javaArgs,
					envVars,
				}}
			/>
		</>
	);
}

export default SettingsMinecraft;

function ClusterJavaVersionModal(props: ModalProps & {
	clusterId: string;
	javaVersion: CreateSetting<JavaVersion | null>;
	javaVersions: CreateSetting<JavaVersions>;
}) {
	const [_, modalProps] = splitProps(props, ['javaVersion', 'javaVersions']);

	const javaVersions = createMemo(() => Object.entries(props.javaVersions.get()).sort((a, b) => Number.parseFloat(b[1].version) - Number.parseFloat(a[1].version)));
	const [selected, setSelected] = createSignal<number>(0);

	onMount(async () => {
		const javaVersion = props.javaVersion.get();
		if (!javaVersion) {
			const clusterId = props.clusterId;
			const optimal = await tryResult(() => bridge.commands.getOptimalJavaVersion(clusterId));
			setSelected(javaVersions().findIndex(([_, meta]) => meta.path === optimal.path));
		}
		else {
			const found = javaVersions().findIndex(([_, meta]) => meta.path === javaVersion.path);
			setSelected(found);
		}
	});

	const setPackage = (pkg: JavaVersion, version: string) => {
		props.javaVersion.set(pkg);
		props.javaVersions.set({
			...props.javaVersions?.get(),
			[version]: pkg,
		});
	};

	const onChange = (selected: number) => {
		const version = javaVersions()[selected]?.[0];
		if (!version)
			return;

		const meta = props.javaVersions.get()[version];
		if (!meta)
			return;

		setPackage(meta, version);
	};

	return (
		<BaseJavaVersionModal
			{...modalProps}
			setPackage={setPackage}
		>
			<h4>Java Path</h4>
			<div class="flex flex-col items-stretch gap-y-2">
				<Dropdown onChange={onChange} selected={selected}>
					<For each={javaVersions()}>
						{([version]) => {
							const major = version.toLowerCase().replaceAll('_', ' ').replaceAll('java', '');

							return (
								<Dropdown.Row>
									<span class="capitalize">
										Java
										{' '}
										{major}
									</span>
								</Dropdown.Row>
							);
						}}
					</For>
				</Dropdown>
			</div>
		</BaseJavaVersionModal>
	);
}

function GlobalJavaVersionModal(props: ModalProps & {
	javaVersions: CreateSetting<JavaVersions>;
}) {
	const [_, modalProps] = splitProps(props, ['javaVersions']);

	const setPackage = (pkg: JavaVersion, version: string) => {
		props.javaVersions?.set({
			...props.javaVersions?.get(),
			[version]: pkg,
		});
	};

	return (
		<BaseJavaVersionModal
			{...modalProps}
			setPackage={setPackage}
		>
			<h4>Default Java Paths</h4>
			<div class="grid grid-cols-[80px_1fr] items-center gap-y-2">
				<For each={Object.entries(props.javaVersions?.get() ?? {}).sort((a, b) => Number.parseFloat(b[1].version) - Number.parseFloat(a[1].version))}>
					{([version, meta]) => {
						const major = version.toLowerCase().replaceAll('_', ' ').replaceAll('java', '');

						return (
							<>
								<span class="capitalize">
									Java
									{' '}
									{major}
								</span>
								<TextField
									onValidSubmit={(value) => {
										setPackage({ ...meta, path: value }, major);
									}}
									value={meta.path}
								/>
							</>
						);
					}}
				</For>
			</div>
		</BaseJavaVersionModal>
	);
}

function BaseJavaVersionModal(props: ModalProps & {
	setPackage: (pkg: JavaVersion, version: string) => void;
}) {
	const [selectedPackageIndex, setSelectedPackageIndex] = createSignal<number>(-1);

	const [zuluPackages] = createResource(async () => {
		try {
			return await tryResult(bridge.commands.getZuluPackages);
		}
		catch (e) {
			console.error(e);
			return [];
		}
	});

	const foundVersions = createMemo(() => {
		return zuluPackages()?.map(pkg => pkg.java_version.join('.')) ?? [];
	});

	const download = async (pkg: JavaZuluPackage) => {
		const path = await tryResult(() => bridge.commands.installJavaFromPackage(pkg));
		if (path)
			props.setPackage({
				version: pkg.java_version.join('.'),
				path,
				arch: '',
			}, pkg.java_version[0]!.toString());
	};

	return (
		<Modal.Simple
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Close"
					onClick={props.hide}
				/>,
			]}
			title="Java Versions"
			{...props}
		>
			<div class="flex flex-col gap-6">
				<div class="flex flex-col gap-2">
					{props.children}
				</div>

				<div class="flex flex-col gap-2">
					<h4>Download Java</h4>
					<div class="w-full flex flex-row gap-2">
						<Dropdown class="w-full" onChange={setSelectedPackageIndex}>
							<For each={foundVersions()}>
								{version => (
									<Dropdown.Row>{version}</Dropdown.Row>
								)}
							</For>
						</Dropdown>

						<Button
							children="Download"
							iconLeft={<Download01Icon />}
							onClick={() => download(zuluPackages()![selectedPackageIndex()]!)}
						/>
					</div>
				</div>
			</div>
		</Modal.Simple>
	);
}
