import type { LauncherError } from '@/bindings.gen';
import type { bindings } from '@/main';

export type BindingCommands = (keyof typeof bindings)[keyof typeof bindings];

declare module '@onelauncher/common' {
	interface Register {
		commands: Array<BindingCommands>;
		defaultError: LauncherError;
	}
}
