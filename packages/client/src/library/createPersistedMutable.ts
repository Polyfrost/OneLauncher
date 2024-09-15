import type { StoreNode } from 'solid-js/store';
import { trackDeep } from '@solid-primitives/deep';
import { createEffect, createRoot } from 'solid-js';

export interface CreatePersistedMutableOpts<T> {
	onSave?: (value: T) => T;
}

// copied from `makePersisted` in `@solid-primitives/storage` with `solid-js/store`.
export function createPersistedMutable<T extends StoreNode>(
	key: string,
	mutable: T,
	opts?: CreatePersistedMutableOpts<T>,
): T {
	try {
		const value = localStorage.getItem(key);
		if (value) {
			const persisted = JSON.parse(value);
			Object.assign(mutable, persisted);
		}
	}
	catch (err) {
		console.error(`failed to load persisted state from localStorage '${key}': ${err}`);
	}

	const dispose = createRoot((dispose) => {
		createEffect(() => {
			trackDeep(mutable);
			const item = opts?.onSave
				? JSON.stringify(opts.onSave(mutable))
				: JSON.stringify(mutable);

			localStorage.setItem(key, item);
		});
		return dispose;
	});

	if ('onHotReload' in globalThis)
		globalThis?.onHotReload(dispose);

	return mutable;
}
