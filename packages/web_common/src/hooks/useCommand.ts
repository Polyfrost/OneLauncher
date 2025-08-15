/* eslint-disable query/exhaustive-deps -- False */
import type { UndefinedInitialDataOptions, UseMutationOptions, UseMutationResult, UseQueryResult, UseSuspenseQueryOptions, UseSuspenseQueryResult } from '@tanstack/react-query';
import type { AppError } from '../utils/error';
import { useMutation, useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { isLauncherError } from '../utils/error';

// this gets overwritten by the consumer project
export interface Register {
	// commands: string[];
	// defaultError: Error;
}

type CommandKeys = Register extends {
	commands: infer Keys;
} ? Keys : never;

type DefaultError = Register extends {
	defaultError: infer Error;
} ? Error : never;

type QueryKey = [CommandKeys[number], ...ReadonlyArray<unknown>];

declare module '@tanstack/react-query' {
	interface Register {
		queryKey: QueryKey;
		mutationKey: QueryKey;
	}
}

async function fetchCommand<T>(command: () => Promise<T>): Promise<T> {
	try {
		const result = await command();
		return result;
	}
	// TODO: (lynith) handle errors better (requires modifications to TauRPC)
	catch (e) {
		console.error('[fetch]', e);
		if (isLauncherError(e))
			return Promise.reject(e);
		else
			return Promise.reject(e);
	}
}

export function useCommand<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?: Omit<
		UndefinedInitialDataOptions<TQueryFnData, TError, TData, TQueryKey>,
		'queryKey' | 'queryFn'
	>,
): UseQueryResult<TData, TError> {
	return useQuery<TQueryFnData, TError, TData, TQueryKey>({
		queryKey: cacheKey,
		queryFn: () => fetchCommand(command),
		enabled: true,
		...options,
	});
}

export function useCommandSuspense<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?: Omit<
		UseSuspenseQueryOptions<TQueryFnData, TError, TData, TQueryKey>,
    'queryKey' | 'queryFn'
	>,
): UseSuspenseQueryResult<TData, TError> {
	return useSuspenseQuery<TQueryFnData, TError, TData, TQueryKey>({
		queryKey: cacheKey,
		queryFn: () => fetchCommand(command),
		...options,
	});
}

export function useCommandMut<
	TData,
	TError = DefaultError,
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
