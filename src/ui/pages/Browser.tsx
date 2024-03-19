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
				onClick={() => {
					invoke('plugin:game|set_selected_client', {
						details: {
							uuid: uuid.v4(),
							name: 'my vanilla instance',
							version: '1.8.9',
							main_class: 'net.minecraft.client.main.Main',
							java_version: 'v8',
							startup_args: ['--username', 'player', '--password', 'password'],
							client_type: {
								type: 'Vanilla',
								manifest: {},
							},
						} as game.GameClientDetails,
					});
				}}
			>
				Test Button2
			</Button>
		</div>
	);
}

export default BrowserPage;
