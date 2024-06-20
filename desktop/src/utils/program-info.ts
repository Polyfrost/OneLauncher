import type { ProgramInfo } from '~bindings';
import { bridge } from '~index';

// eslint-disable-next-line import/no-mutable-exports -- because yes
export let AppInfo!: ProgramInfo;
export default AppInfo;

export async function initAppInfo() {
	AppInfo = await bridge.commands.getProgramInfo();
}
