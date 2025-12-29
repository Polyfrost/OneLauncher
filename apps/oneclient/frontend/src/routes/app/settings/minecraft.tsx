import type { SettingProfileModel } from '@/bindings.gen';
import { SettingsRow } from '@/components';
import usePopState from '@/hooks/usePopState';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Sidebar } from '@/routes/app/settings/route';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Switch, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Database01Icon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, XIcon } from '@untitled-theme/icons-react';
import { useMemo } from 'react';

export const Route = createFileRoute('/app/settings/minecraft')({
	component: RouteComponent,
});

function RouteComponent() {
	const result = useCommandSuspense(['readSettings'], bindings.core.readSettings);

	const save = useCommandMut(() => bindings.core.writeSettings(result.data));

	usePopState(() => {
		save.mutate();
	});

	return (
		<Sidebar.Page>
			{/* <ScrollableContainer> */}
			<div className="h-full">
				<h1 className="text-xl">Minecraft Settings</h1>

				<GameSettings />

				{/* <LauncherSettings /> */}

				<ProcessSettings />
			</div>
			{/* </ScrollableContainer> */}
		</Sidebar.Page>
	);
}

export function GameSettings() {
	const { createSetting } = useSettings();

	const [gameSettings, setGameSettings] = createSetting('global_game_settings');

	// const getSetting = useMemo(()=><TKey extends keyof SettingProfileModel>(key: TKey)=>gameSettings[key], [gameSettings])

	const setSetting = useMemo(() => <TKey extends keyof SettingProfileModel>(key: TKey, value: SettingProfileModel[TKey]) => {
		setGameSettings({
			...gameSettings,
			[key]: value,
		});
	}, [gameSettings, setGameSettings]);

	return (
		<>
			<SettingsRow.Header>Game</SettingsRow.Header>

			<SettingsRow
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
				title="Force Fullscreen"
			>
				<Switch
					isSelected={gameSettings.force_fullscreen ?? false}
					onChange={val => setSetting('force_fullscreen', val)}
				/>
			</SettingsRow>

			<SettingsRow
				description="The game window resolution in pixels."
				icon={<LayoutTopIcon />}
				title="Resolution"
			>
				<div className="grid grid-cols-[70px_16px_70px] gap-2 grid-justify-center grid-items-center">
					<TextField
						className="text-center"
						min={1}
						onChange={(e) => {
							const width = Math.max(1, Number(e.currentTarget.value));
							setSetting('res', { width, height: gameSettings.res?.height ?? 720 });
						}}
						type="number"
						value={gameSettings.res?.width ?? 1280}
					/>
					<XIcon className="size-4 self-center" />
					<TextField
						className="text-center"
						min={1}
						onChange={(e) => {
							const height = Math.max(1, Number(e.currentTarget.value));
							setSetting('res', { width: gameSettings.res?.width ?? 1280, height });
						}}
						type="number"
						value={gameSettings.res?.height ?? 720}
					/>
				</div>
			</SettingsRow>

			<SettingsRow
				description="The amount of memory in megabytes allocated for the game."
				icon={<Database01Icon />}
				title="Memory"
			>
				<div className="flex items-center gap-x-4 flex-justify-center">
					<div className="flex flex-row items-center gap-x-2 relative">
						<TextField
							className="text-center pr-10"
							onBlur={() => {
								setSetting('mem_max', gameSettings.mem_max ?? 0);
							}}
							onChange={(e) => {
								const raw = e.currentTarget.value;
								setSetting('mem_max', raw === '' ? null : Math.max(0, Number(raw)));
							}}
							type="number"
							value={gameSettings.mem_max ?? ''}
						/>
						<div className="absolute inset-y-0 right-3 flex items-center">
							<p className="text-sm text-fg-secondary">MB</p>
						</div>
					</div>
				</div>
			</SettingsRow>
		</>
	);
}

// function LauncherSettings() {
// 	return (
// 		<>
// 			<SettingsRow.Header>Launcher</SettingsRow.Header>
// 			<SettingsRow
// 				description="Hide the launcher whenever you start a game."
// 				icon={<EyeIcon />}
// 				title="Hide On Launch"
// 			>
// 				<ToggleButton children="false" color="primary" />
// 			</SettingsRow>
// 		</>
// 	);
// }

export function ProcessSettings() {
	const { createSetting } = useSettings();

	const [gameSettings, setGameSettings] = createSetting('global_game_settings');

	// const getSetting = useMemo(()=><TKey extends keyof SettingProfileModel>(key: TKey)=>gameSettings[key], [gameSettings])

	const setSetting = useMemo(() => <TKey extends keyof SettingProfileModel>(key: TKey, value: SettingProfileModel[TKey]) => {
		setGameSettings({
			...gameSettings,
			[key]: value,
		});
	}, [gameSettings, setGameSettings]);

	return (
		<>
			<SettingsRow.Header>Process</SettingsRow.Header>

			<SettingsRow
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
				title="Pre-Launch Command"
			>
				<TextField
					onChange={e => setSetting('hook_pre', e.currentTarget.value)}
					placeholder="echo 'Game started'"
					value={gameSettings.hook_pre ?? undefined}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run when launching the game."
				icon={<ParagraphWrapIcon />}
				title="Wrapper Command"
			>
				<TextField
					onChange={e => setSetting('hook_wrapper', e.currentTarget.value)}
					placeholder="gamescope"
					value={gameSettings.hook_wrapper ?? undefined}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run after exiting the game."
				icon={<FileX02Icon />}
				title="Post-Exit Command"
			>
				<TextField
					onChange={e => setSetting('hook_post', e.currentTarget.value)}
					placeholder="echo 'Game exited'"
					value={gameSettings.hook_post ?? undefined}
				/>
			</SettingsRow>
		</>
	);
}
