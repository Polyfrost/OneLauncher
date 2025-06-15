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

export function useCommand<T>(
	cacheKey: RegisterConfig[number] | (string & {}) | false,
	command: () => Promise<T>,
	options?: Omit<UndefinedInitialDataOptions<T>, 'queryKey' | 'queryFn'>,
): UseQueryResult<T, Error> {
	return useQuery({
		queryKey: cacheKey === false ? [] : [cacheKey],
		queryFn: () => fetchCommand(command),
		enabled: true,
		...options,
	});
}

export function useCommandSuspense<T>(
	cacheKey: RegisterConfig[number] | (string & {}) | false,
	command: () => Promise<T>,
	options?: Omit<UseSuspenseQueryOptions<T>, 'queryKey' | 'queryFn'>,
): UseSuspenseQueryResult<T, Error> {
	return useSuspenseQuery({
		queryKey: cacheKey === false ? [] : [cacheKey],
		queryFn: () => fetchCommand(command),
		...options,
	});
}

export function useCommandMut<TData, TError = Error, TVariables = void, TContext = unknown>(
	command: () => Promise<TData>,
	options?: Omit<UseMutationOptions<TData, TError, TVariables, TContext>, 'mutationFn'>,
): UseMutationResult<TData, TError, TVariables, TContext> {
	return useMutation<TData, TError, TVariables, TContext>({
		mutationFn: () => fetchCommand(command),
		...options,
	});
}
