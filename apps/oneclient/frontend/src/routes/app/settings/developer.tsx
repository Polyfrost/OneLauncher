import { DebugInfo, Overlay } from '@/components/overlay';
import SettingsRow from '@/components/SettingsRow';
import SettingsSwitch from '@/components/SettingSwitch';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';
import { BatteryEmptyIcon, BatteryFullIcon, Code02Icon, FileHeart02Icon, Truck01Icon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/developer')({
	component: RouteComponent,
});

function RouteComponent() {
	const { createSetting } = useSettings();

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1 className="text-xl">Developer Options</h1>

				<SettingsRow
					description="Open Debug Info"
					icon={<FileHeart02Icon />}
					title="Open Debug Info"
				>
					<Overlay.Trigger>
						<Button size="normal">Open</Button>

						<Overlay>
							<DebugInfo />
						</Overlay>
					</Overlay.Trigger>
				</SettingsRow>

				<SettingsRow.Header>Dev Tools</SettingsRow.Header>
				<SettingsRow
					description="Enable The Tanstack Dev Tools"
					icon={<Code02Icon />}
					title="Tanstack Dev Tools"
				>
					<SettingsSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>
				<SettingsRow
					description="Open Dev Tools"
					icon={<Code02Icon />}
					title="Open Dev Tools"
				>
					<Button onPress={bindings.debug.openDevTools} size="normal">Open</Button>
				</SettingsRow>

				<SettingsRow.Header>Onboarding</SettingsRow.Header>
				<SettingsRow
					description="Open Onboarding"
					icon={<Truck01Icon />}
					title="Open Onboarding"
				>
					<Link to="/onboarding">
						<Button size="normal">Open</Button>
					</Link>
				</SettingsRow>
				<SettingsRow
					description="Seen onboarding"
					icon={<Truck01Icon />}
					title="Seen Onboarding"
				>
					<SettingsSwitch setting={createSetting('seen_onboarding')} />
				</SettingsRow>
				<SettingsRow
					description="Use Grid On Mods List"
					icon={<BatteryFullIcon />}
					title="Use Grid On Mods List"
				>
					<SettingsSwitch setting={createSetting('mod_list_use_grid')} />
				</SettingsRow>

				<SettingsRow.Header>Mod Downloading</SettingsRow.Header>
				<SettingsRow description="Use Parallel Mod Downloading for speed. This can create some issues sometimes" icon={<BatteryEmptyIcon />} title="Use Parallel Mod Downloading">
					<SettingsSwitch setting={createSetting('parallel_mod_downloading')} />
				</SettingsRow>

			</div>
		</Sidebar.Page>
	);
}
