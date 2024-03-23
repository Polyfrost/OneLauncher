import { invoke } from '@tauri-apps/api/core';
import * as uuid from 'uuid';
import Button from '../components/base/Button';

function BrowserPage() {
	return (
		<div class="flex flex-col gap-y-4">
			<h1>Browser</h1>
			<Button
				// eslint-disable-next-line solid/reactivity
				onClick={async () => {
					await invoke('plugin:game|launch_game');
					// eslint-disable-next-line no-console
					console.log(`Game exited`);
				}}
			>
				Test Button
			</Button>

			<Button
				onClick={async () => {
					const result = invoke('plugin:game|get_instances');
					console.log(result);
				}}
			>
				Test Button 2
			</Button>

			<Button
				onClick={async () => {
					const result = await invoke('plugin:game|create_instance', {
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
