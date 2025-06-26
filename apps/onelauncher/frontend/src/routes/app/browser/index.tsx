import Modal from '@/components/overlay/Modal';
import useNotifications from '@/hooks/useNotification';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Dropdown, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/browser/')({
	component: RouteComponent,
});

function RouteComponent() {
	const { create, list, clear } = useNotifications();
	const result = useCommand('read_settings', bindings.core.readSettings);

	const addSimpleNotif = () => {
		create({
			message: 'This is a simple notification message',
			title: 'Simple Notification',
		});
	};

	const addProgressNotif = () => {
		create({
			message: 'Download in progress...',
			title: 'Downloading File',
			fraction: Math.floor(Math.random() * 100),
		});
	};

	const addErrorNotif = () => {
		create({
			message: 'Something went wrong while processing your request',
			title: 'Error',
		});
	};

	return (
		<div>
			<p>asdasdas</p>

			<TextField />

			{/* we can open the modal but we cant close it for some reason */}
			<Modal.Trigger>
				<Button>Open Modal</Button>

				<Modal>
					<p>sadsadsad</p>
				</Modal>
			</Modal.Trigger>

			<Dropdown label="Select a version">
				<Dropdown.Item>Slmalr</Dropdown.Item>
				<Dropdown.Item>Slmalr 2</Dropdown.Item>
			</Dropdown>

			<div className="flex gap-2 flex-wrap">
				<Button onClick={addSimpleNotif}>Add Simple Notification</Button>
				<Button onClick={addProgressNotif}>Add Progress Notification</Button>
				<Button onClick={addErrorNotif}>Add Error Notification</Button>
				<Button onClick={clear}>Clear All Notifications</Button>
			</div>

			<div className="mt-4">
				<h3 className="font-semibold mb-2">
					Current Notifications (
					{Object.keys(list).length}
					):
				</h3>
				<pre className="text-xs bg-component-bg-hover p-2 rounded overflow-auto max-h-40">
					{JSON.stringify(list, null, 2)}
				</pre>
			</div>

			<div className="mt-4">
				<h3>Settings</h3>
				<pre className="text-xs bg-component-bg-hover p-2 rounded overflow-auto max-h-40">
					{JSON.stringify(result.data, null, 2)}
				</pre>
			</div>
		</div>
	);
}
