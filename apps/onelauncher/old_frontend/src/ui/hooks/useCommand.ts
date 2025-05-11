import type { Result } from '@onelauncher/client/bindings';
import { createResource, type ResourceOptions, type ResourceReturn, type ResourceSource } from 'solid-js';

type Command<R, E> = () => Promise<Result<R, E>>;

export function useCommand<R, E = string>(
	cmd: Command<R, E>,
	options?: ResourceOptions<R>,
): ResourceReturn<R>;

export function useCommand<R, S, E = string>(
	source: ResourceSource<S>,
	cmd: Command<R, E>,
	options?: ResourceOptions<R>,
): ResourceReturn<R>;

export default function useCommand<R, S, E = string>(
	pSource: ResourceSource<S> | Command<R, E>,
	pCmd?: Command<R, E> | ResourceOptions<R>,
	options?: ResourceOptions<R>,
): ResourceReturn<R> {
	let source: ResourceSource<S>;
	let command: Command<R, E>;
	let opts: ResourceOptions<R> | undefined;

	switch (arguments.length) {
		case 1: {
			source = true as ResourceSource<S>;
			command = pSource as Command<R, E>;
			opts = {} as ResourceOptions<R>;
			break;
		}
		case 2: {
			if (pCmd instanceof Function) {
				source = pSource as ResourceSource<S>;
				command = pCmd;
				opts = {} as ResourceOptions<R>;
			}
			else {
				source = true as ResourceSource<S>;
				command = pSource as Command<R, E>;
				opts = pCmd as ResourceOptions<R>;
			}
			break;
		}
		case 3: {
			source = pSource as ResourceSource<S>;
			command = pCmd! as Command<R, E>;
			opts = options as ResourceOptions<R>;
			break;
		}
	}

	return createResource(source, async () => await tryResult(command), opts);
}

export async function tryResult<R, E>(
	cmd: () => Promise<Result<R, E>>,
): Promise<R> {
	const value = await cmd();

	if (value.status === 'ok')
		return value.data;

	throw value.error;
}
