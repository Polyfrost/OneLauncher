import SettingsRow from '@/components/SettingsRow';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Link } from '@tanstack/react-router';
import { Truck01Icon } from '@untitled-theme/icons-react';
import Sidebar from './route';

export const Route = createFileRoute('/app/settings/developer')({
	component: RouteComponent,
})

function RouteComponent() {
	return (
		<Sidebar.Page>
			<div className="h-full">
				<h1>General</h1>

				<SettingsRow.Header>Folders and Files</SettingsRow.Header>
				<SettingsRow
					description="Open onboarding"
					icon={<Truck01Icon />}
					title="Onboarding"
				>
					<Link to="/onboarding">
						<Button size="normal">Open</Button>
					</Link>
				</SettingsRow>
			</div>
		</Sidebar.Page>
	);
}
