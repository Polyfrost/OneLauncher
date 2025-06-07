import type { bindings } from '@/main';

export type BindingCommands = (keyof typeof bindings.core) | (keyof typeof bindings.onelauncher);

declare module '@onelauncher/common' {
	interface Register {
		commands: Array<BindingCommands>;
	}
}
