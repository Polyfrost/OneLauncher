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

	setBodyAttributes();
}

export const getProgramInfo = () => _programInfo;

function setBodyAttributes() {
	document.body.setAttribute('data-platform', _programInfo.platform);
	if (_programInfo.platform === 'linux') {
		const minorVersion = Number.parseInt(_programInfo.webview_version.split('.')[1] || 'NaN');

		// Check if WebKitGTK version is 2.46 or higher (Skia renderer implemented which fixes some issues)
		document.body.setAttribute('data-skia-renderer', (!Number.isNaN(minorVersion) && minorVersion >= 46).toString());
	}
}
