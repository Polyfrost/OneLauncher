import { commands, type ProgramInfo } from '../bindings';

let _programInfo: ProgramInfo = {
	arch: 'unknown',
	platform: 'unknown',
	dev_build: true,
	launcher_version: 'unknown',
	tauri_version: 'unknown',
	webview_version: 'unknown',
};

export async function initProgramInfo() {
	const result = await commands.getProgramInfo();
	if (result.status === 'ok')
		_programInfo = result.data;
}

export const getProgramInfo = () => _programInfo;
