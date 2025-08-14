/* eslint-disable query/exhaustive-deps -- False */
import type { UndefinedInitialDataOptions, UseMutationOptions, UseMutationResult, UseQueryResult, UseSuspenseQueryOptions, UseSuspenseQueryResult } from '@tanstack/react-query';
import { useMutation, useQuery, useSuspenseQuery } from '@tanstack/react-query';

// this gets overwritten by the consumer project
export interface Register {
	// commands
}

type RegisterConfig = Register extends {
	commands: infer Keys;
} ? Keys : never;

type CommandCacheKey = RegisterConfig[number] | (string & {});
type CommandCacheKeys = Array<CommandCacheKey> | [CommandCacheKey] | [false];

async function fetchCommand<T>(command: () => Promise<T>): Promise<T> {
	try {
		const result = await command();
		return result;
	}
	// TODO: (lynith) handle errors better (requires modifications to TauRPC)
	catch (e) {
		console.error(e);
		if (e instanceof Error)
			return Promise.reject(e);
		else
			return Promise.reject(new Error('Unknown error'));
	}
}

export function useCommand<TQueryFnData, TSelected = TQueryFnData>(
	cacheKey: CommandCacheKeys,
	command: () => Promise<TQueryFnData>,
	options?: Omit<
		UndefinedInitialDataOptions<TQueryFnData, Error, TSelected, CommandCacheKeys | []>,
		'queryKey' | 'queryFn'
	>,
): UseQueryResult<TSelected, Error> {
	return useQuery<TQueryFnData, Error, TSelected, CommandCacheKeys | []>({
		queryKey: cacheKey[0] === false ? [] : Array.isArray(cacheKey) ? cacheKey : [cacheKey],
		queryFn: () => fetchCommand(command),
		enabled: true,
		...options,
	});
}

export function useCommandSuspense<TQueryFnData, TSelected = TQueryFnData>(
	cacheKey: CommandCacheKeys,
	command: () => Promise<TQueryFnData>,
	options?: Omit<
		UseSuspenseQueryOptions<TQueryFnData, Error, TSelected, CommandCacheKeys | []>,
    'queryKey' | 'queryFn'
	>,
): UseSuspenseQueryResult<TSelected, Error> {
	return useSuspenseQuery<TQueryFnData, Error, TSelected, CommandCacheKeys | []>({
		queryKey: cacheKey[0] === false ? [] : Array.isArray(cacheKey) ? cacheKey : [cacheKey],
		queryFn: () => fetchCommand(command),
		...options,
	});
}

export function useCommandMut<
	TData,
	TError = Error,
	TVariables = void,
	TContext = unknown,
>(
	command: (variables: TVariables) => Promise<TData>,
	options?: Omit<
		UseMutationOptions<TData, TError, TVariables, TContext>,
		'mutationFn'
	>,
): UseMutationResult<TData, TError, TVariables, TContext> {
	return useMutation<TData, TError, TVariables, TContext>({
		mutationFn: command,
		...options,
	});
}
