import { SettingDropdown, SettingNumber, SettingsRow, SettingSwitch } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { Sidebar } from '@/routes/app/settings/route';
import { ToastPositions, useToast } from '@/utils/toast';
import { Button } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/settings/appearance')({
	component: RouteComponent,
});

function RouteComponent() {
	const { createSetting } = useSettings();
	const toast = useToast();

	const previewToast = () => toast({ type: 'info', title: 'Preview Toast', message: 'This is an preview of what a toast would look like' });

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1 className="text-xl">Appearance</h1>

				<SettingsRow.Header>Toasts</SettingsRow.Header>

				<SettingsRow description="Where the toast will show up" title="Position">
					<SettingDropdown options={ToastPositions} setting={createSetting('toast_position')} />
				</SettingsRow>

				<SettingsRow description="Should the toast expire and auto close" title="Auto Close">
					<SettingSwitch setting={createSetting('toast_auto_close')} />
				</SettingsRow>

				<SettingsRow description="How long the toast should stay" title="Duration">
					<SettingNumber max={60000} min={500} setting={createSetting('toast_duration')} />
				</SettingsRow>

				<SettingsRow description="Preview a Toast" title="Preview Toast">
					<Button onClick={previewToast} size="normal">Preview</Button>
				</SettingsRow>

			</div>
		</Sidebar.Page>
	);
}
