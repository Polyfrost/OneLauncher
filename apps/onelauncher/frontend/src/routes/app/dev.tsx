import Modal from '@/components/overlay/Modal';
import useNotifications from '@/hooks/useNotification';
import useSettings from '@/hooks/useSettings';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { Button, Dropdown, Switch, TextField } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/app/dev')({
	component: RouteComponent,
});

function RouteComponent() {
	const { create, list, clear } = useNotifications();
	const { settings, createSetting } = useSettings();
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

	const [parallel, setParallel] = createSetting('allow_parallel_running_clusters', settings?.allow_parallel_running_clusters);

	return (
		<div>
			<p>this is really became my test place fr</p>

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

			<div className="flex flex-row">
				<div className="w-1/2">
					<div className="mt-4">
						<h3>Settings</h3>
						<pre className="text-xs bg-component-bg-hover p-2 rounded overflow-auto max-h-40">
							{JSON.stringify(result.data, null, 2)}
						</pre>
					</div>

					<div className="mt-4">
						<h3>Settings again but this time its from useSettings hook</h3>
						<pre className="text-xs bg-component-bg-hover p-2 rounded overflow-auto max-h-40">
							{JSON.stringify(settings, null, 2)}
						</pre>
					</div>
				</div>

				<div className="w-1/2">
					<div className="ml-2">
						<pre>
							{parallel ? 'Parallel running is enabled' : 'Parallel running is disabled'}
						</pre>
						<Switch defaultSelected={parallel} onChange={val => setParallel(val)} />
					</div>
				</div>
			</div>
		</div>
	);
}
