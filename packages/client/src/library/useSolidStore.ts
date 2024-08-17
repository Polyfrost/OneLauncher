import type { Store } from 'solid-js/store';
import { useObserver } from './useObserver';

/**
 * A wrapper for {@link useObserver} to integrate with `solid-js/store`.
 */
export function useSolidStore<T extends object = object>(store: Store<T>): T {
	const state = useObserver(() => ({ ...store }));
	return new Proxy(state, {
		get: (target, prop) => Reflect.get(target, prop),
		set: (_, prop, value) => Reflect.set(store, prop, value),
	});
}
