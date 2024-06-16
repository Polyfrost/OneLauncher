import { type ResourceReturn, createResource } from 'solid-js';
import type { Result } from '~bindings';

/**
 * A helper function which handles a Rust Result properly
 * @param cmd The command to call
 * @throws
 */
export default function useCommand<R, E, Args extends unknown[]>(
	cmd: (...args: Args) => Promise<Result<R, E>>,
	...args: Args
): ResourceReturn<R> {
	return createResource(async () => {
		const value = await cmd(...args);

		if (value.status === 'ok')
			return value.data;

		throw value.error;
	});
}
