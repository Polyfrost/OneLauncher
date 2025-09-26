/* eslint-disable query/exhaustive-deps -- False */
import type { DefinedInitialDataOptions, DefinedUseQueryResult, FetchQueryOptions, UndefinedInitialDataOptions, UseMutationOptions, UseMutationResult, UseQueryResult, UseSuspenseQueryOptions, UseSuspenseQueryResult } from '@tanstack/react-query';
import type { AppError } from '../utils/error';
import { useMutation, usePrefetchQuery, useQuery, useSuspenseQuery } from '@tanstack/react-query';
import { isLauncherError } from '../utils/error';

// this gets overwritten by the consumer project
export interface Register {
	// commands: string[];
	// defaultError: Error;
}

type CommandKeys = Register extends {
	commands: infer Keys extends ReadonlyArray<string>;
} ? Keys[number] : string;

type DefaultError = Register extends {
	defaultError: infer Error;
} ? Error : never;

type QueryKey = [CommandKeys, ...ReadonlyArray<unknown>];

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

type OmittedOptions<T> = Omit<T, 'queryKey' | 'queryFn'>;

export function useCommand<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options: OmittedOptions<DefinedInitialDataOptions<TQueryFnData, TError, TData, TQueryKey>>,
): DefinedUseQueryResult<TData, TError>;

export function useCommand<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?: OmittedOptions<UndefinedInitialDataOptions<TQueryFnData, TError, TData, TQueryKey>>,
): UseQueryResult<TData, TError>;

export function useCommand<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?:
		| OmittedOptions<DefinedInitialDataOptions<TQueryFnData, TError, TData, TQueryKey>>
		| OmittedOptions<UndefinedInitialDataOptions<TQueryFnData, TError, TData, TQueryKey>>,
) {
	return useQuery<TQueryFnData, TError, TData, TQueryKey>({
		queryKey: cacheKey,
		queryFn: () => fetchCommand(command),
		enabled: true,
		...options,
	});
}

export function usePrefetchedCommand<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?: OmittedOptions<FetchQueryOptions<TQueryFnData, TError, TData, TQueryKey>>,
) {
	return usePrefetchQuery<TQueryFnData, TError, TData, TQueryKey>({
		queryKey: cacheKey,
		queryFn: () => fetchCommand(command),
		...options,
	});
}

export function useCommandSuspense<TQueryFnData = unknown, TError = DefaultError, TData = TQueryFnData, TQueryKey extends QueryKey = QueryKey>(
	cacheKey: TQueryKey,
	command: () => Promise<TQueryFnData>,
	options?: OmittedOptions<UseSuspenseQueryOptions<TQueryFnData, TError, TData, TQueryKey>>,
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
