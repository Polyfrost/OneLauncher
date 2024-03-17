import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type MicrosoftLoginCallback = (msg: string, stage: number) => never;
async function loginMicrosoft(cb?: MicrosoftLoginCallback): Promise<auth.Account> {
	if (cb) {
		const unlisten = await listen<[string, number, boolean]>('msa:status', (event) => {
			cb(event.payload[0], event.payload[1]);

			if (event.payload[2])
				unlisten();
		});
	}

	return await invoke('plugin:auth|login_msa');
}

export default {
	loginMicrosoft,
};
