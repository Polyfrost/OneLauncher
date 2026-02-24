import { DebugInfo, Overlay, SettingsRow, SettingsSwitch } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Sidebar } from '@/routes/app/settings/route';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';

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
				<SettingsRow
					description="Enable The Tanstack Dev Tools"
					title="Tanstack Dev Tools"
				>
					<SettingsSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>
				<SettingsRow
					description="Open Dev Tools"
					title="Open Dev Tools"
				>
					<Button onPress={bindings.debug.openDevTools} size="normal">Open</Button>
				</SettingsRow>

				<SettingsRow.Header>Onboarding</SettingsRow.Header>
				<SettingsRow
					description="Open Onboarding"
					title="Open Onboarding"
				>
					<Link to="/onboarding">
						<Button size="normal">Open</Button>
					</Link>
				</SettingsRow>
				<SettingsRow
					description="Seen onboarding"
					title="Seen Onboarding"
				>
					<SettingsSwitch setting={createSetting('seen_onboarding')} />
				</SettingsRow>
				<SettingsRow
					description="Use Grid On Mods List"
					title="Use Grid On Mods List"
				>
					<SettingsSwitch setting={createSetting('mod_list_use_grid')} />
				</SettingsRow>

			</div>
		</Sidebar.Page>
	);
}
