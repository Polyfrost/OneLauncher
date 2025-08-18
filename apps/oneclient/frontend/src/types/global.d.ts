import type { LauncherError } from '@/bindings.gen';
import type { bindings } from '@/main';

export type BindingCommands = (keyof typeof bindings.core) | (keyof typeof bindings.oneclient);

declare module '@onelauncher/common' {
	interface Register {
		commands: Array<BindingCommands>;
		defaultError: LauncherError;
	}
}
