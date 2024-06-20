import type { ProgramInfo } from '~bindings';
import { bridge } from '~imports';

// eslint-disable-next-line import/no-mutable-exports -- because yes
export let AppInfo: ProgramInfo = {
	launcher_version: 'err',
	tauri_version: 'err',
	webview_version: 'err',
	platform: 'err',
	arch: 'err',
	dev_build: false,
};

export default AppInfo;

export async function initAppInfo() {
	try {
		AppInfo = await bridge.commands.getProgramInfo();
	}
	catch (err) {
		console.error(err);
	}
}
