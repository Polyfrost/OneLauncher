import { invoke } from '@tauri-apps/api/core';
import Button from '../components/base/Button';

function BrowserPage() {
	return (
		<div class="flex flex-col gap-y-4">
			<h1>Browser</h1>
			<Button
				// eslint-disable-next-line solid/reactivity
				onClick={async () => {
					const result = await invoke('plugin:launcher-core|get_instance', {
						uuid: 'de69f608-c8de-4cb5-a295-8747dc05380a',
					});
					// eslint-disable-next-line no-console
					console.log(result);
				}}
			>
				Test Button
			</Button>

			<Button
				onClick={async () => {
					const result = await invoke('plugin:launcher-core|get_instances');
					console.log(result);
				}}
			>
				Test Button 2
			</Button>

			<Button
				onClick={async () => {
					const result = await invoke('plugin:launcher-core|create_instance', {
						name: 'My epic instance name',
						version: '1.8.9',
						client: {
							type: 'Vanilla',
						},
					});

					console.log(result);
				}}
			>
				Test Button 3
			</Button>
		</div>
	);
}

export default BrowserPage;
