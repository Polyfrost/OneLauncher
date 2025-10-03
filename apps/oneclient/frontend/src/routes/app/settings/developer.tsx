import SettingsRow from '@/components/SettingsRow';
import SettingsSwitch from '@/components/SettingSwitch';
import { useSettings } from '@/hooks/useSettings';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';
import { Truck01Icon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/developer')({
	component: RouteComponent,
});

function RouteComponent() {
	const { createSetting } = useSettings();

	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1>Onboarding</h1>

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

			</div>
		</Sidebar.Page>
	);
}
