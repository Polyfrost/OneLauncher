import { SheetPage } from '@/components';
import SettingsRow from '@/components/SettingsRow';
import { useClusterProfile } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Switch, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { Database01Icon, FilePlus02Icon, FileX02Icon, LayoutTopIcon, Maximize01Icon, ParagraphWrapIcon, XIcon } from '@untitled-theme/icons-react';

export const Route = createFileRoute('/app/cluster/settings')({
	component: RouteComponent,
});

function RouteComponent() {
	const { cluster } = Route.useRouteContext();
	const { profile, updateProfile } = useClusterProfile(cluster);
	return (
		<SheetPage.Content>
			<h1>Settings</h1>
			<SettingsRow.Header>Process</SettingsRow.Header>

			<SettingsRow
				description="Command to run before launching the game."
				icon={<FilePlus02Icon />}
				title="Pre-Launch Command"
			>
				<TextField
					onChange={e => updateProfile({ hook_pre: e.currentTarget.value })}
					placeholder="echo 'Game started'"
					value={profile.hook_pre ?? undefined}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run when launching the game."
				icon={<ParagraphWrapIcon />}
				title="Wrapper Command"
			>
				<TextField
					onChange={e => updateProfile({ hook_wrapper: e.currentTarget.value })}
					placeholder="gamescope"
					value={profile.hook_wrapper ?? undefined}
				/>
			</SettingsRow>

			<SettingsRow
				description="Command to run after exiting the game."
				icon={<FileX02Icon />}
				title="Post-Exit Command"
			>
				<TextField
					onChange={e => updateProfile({ hook_post: e.currentTarget.value })}
					placeholder="echo 'Game exited'"
					value={profile.hook_post ?? undefined}
				/>
			</SettingsRow>

			<SettingsRow.Header>Game</SettingsRow.Header>

			<SettingsRow
				description="Force Minecraft to start in fullscreen mode."
				icon={<Maximize01Icon />}
				title="Force Fullscreen"
			>
				<Switch
					isSelected={profile.force_fullscreen ?? false}
					onChange={val => updateProfile({ force_fullscreen: val })}
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
						onChange={(e) => {
							updateProfile({ res: { height: profile.res?.height ?? 720, width: Number(e.currentTarget.value) } });
						}}
						type="number"
						value={profile.res?.width}
					/>
					<XIcon className="size-4 self-center" />
					<TextField
						className="text-center"
						onChange={(e) => {
							updateProfile({ res: { width: profile.res?.height ?? 1280, height: Number(e.currentTarget.value) } });
						}}
						type="number"
						value={profile.res?.height}
					/>
				</div>
			</SettingsRow>

			<SettingsRow
				description="The amount of memory in megabytes allocated for the game."
				icon={<Database01Icon />}
				title="Memory"
			>
				<div className="flex items-center gap-x-4 flex-justify-center">
					<div className="flex flex-row items-center gap-x-2">
						<TextField
							className="text-center"
							onChange={(e) => {
								updateProfile({ mem_max: Number(e.currentTarget.value) });
							}}
							type="number"
							value={profile.mem_max ?? undefined}
						/>
					</div>
				</div>
			</SettingsRow>
		</SheetPage.Content>
	);
}
