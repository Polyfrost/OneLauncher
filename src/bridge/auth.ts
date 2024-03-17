import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type MicrosoftLoginCallback = (msg: string, stage: number) => never;
export async function loginMicrosoft(cb?: MicrosoftLoginCallback): Promise<auth.Account> {
	if (cb) {
		const unlisten = await listen<[string, number, boolean]>('msa:status', (event) => {
			cb(event.payload[0], event.payload[1]);

			if (event.payload[2])
				unlisten();
		});
	}

	return await invoke('plugin:auth|login_msa');
}

export async function getUuidHeadSrc(uuid: string): Promise<string> {
	// TODO implement some kind of caching + fallback
	return `https://crafatar.com/avatars/${uuid}?size=32`;
}

export default {
	loginMicrosoft,
	getUuidHeadSrc,
};
