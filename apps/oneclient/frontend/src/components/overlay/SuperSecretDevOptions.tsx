import { Overlay, SettingsRow, SettingSwitch } from '@/components';
import { useCopyDebugInfo } from '@/hooks/useDebugInfo';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { Link } from '@tanstack/react-router';

export function SuperSecretDevOptions() {
	const { createSetting } = useSettings();
	const copyDebugInfo = useCopyDebugInfo();

	return (
		<Overlay.Dialog>
			<Overlay.Title>Super Secret Dev Options</Overlay.Title>

			<div>
				<SettingsRow description="Enable The Tanstack Dev Tools and shows debug page" title="Show Dev stuff">
					<SettingSwitch setting={createSetting('show_tanstack_dev_tools')} />
				</SettingsRow>

				<SettingsRow description="WARNING! This requires a restart to apply. Logs out debug info" title="Log Debug Info">
					<SettingSwitch setting={createSetting('log_debug_info')} />
				</SettingsRow>

				<div className="grid grid-cols-4 gap-3 justify-items-center">
					<Button onPress={bindings.debug.openDevTools} size="normal">Open Dev Tools</Button>

					<Button onPress={copyDebugInfo}>Copy Debug Info</Button>

					<Link to="/app/debug">
						<Button size="normal">Debug Page</Button>
					</Link>

					<Link to="/onboarding/finished">
						<Button size="normal">Skip Onboarding</Button>
					</Link>

				</div>

			</div>
		</Overlay.Dialog>
	);
}
