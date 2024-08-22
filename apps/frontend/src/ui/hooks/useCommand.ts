import { type ResourceReturn, type ResourceSource, createResource } from 'solid-js';
import type { Result } from '@onelauncher/client/bindings';

type Command<R, E> = () => Promise<Result<R, E>>;

export function useCommand<R, E = string>(
	cmd: Command<R, E>,
): ResourceReturn<R>;

export function useCommand<R, S, E = string>(
	source: ResourceSource<S>,
	cmd: Command<R, E>,
): ResourceReturn<R>;

export default function useCommand<R, S, E = string>(
	pSource: ResourceSource<S> | Command<R, E>,
	pCmd?: Command<R, E>,
): ResourceReturn<R> {
	let source: ResourceSource<S>;
	let command: Command<R, E>;

	if (arguments.length === 1) {
		source = true as ResourceSource<S>;
		command = pSource as Command<R, E>;
	}
	else {
		source = pSource as ResourceSource<S>;
		command = pCmd!;
	}

	return createResource(source, async () => await tryResult(command));
}

export async function tryResult<R, E>(
	cmd: () => Promise<Result<R, E>>,
): Promise<R> {
	const value = await cmd();

	if (value.status === 'ok')
		return value.data;

	throw value.error;
}
