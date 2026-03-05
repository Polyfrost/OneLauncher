import { DebugInfo, Overlay, SettingsRow, SettingSwitch } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { Sidebar } from '@/routes/app/settings/route';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

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

				<SettingsRow description="WARNING! This requires a restart to apply. Logs out debug info" title="Log Debug Info">
					<SettingSwitch setting={createSetting('log_debug_info')} />
				</SettingsRow>

				<SettingsRow description="Enable The Tanstack Dev Tools and shows debug page" title="Show Dev stuff">
					<SettingSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>

			</div>
		</Sidebar.Page>
	);
}
