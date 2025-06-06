import type { UndefinedInitialDataOptions, UseQueryResult } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';

// this gets overwritten by the consumer project
export interface Register {
	// commands
}

type RegisterConfig = Register extends {
	commands: infer Keys;
} ? Keys : never;

export function useCommand<T>(cacheKey: RegisterConfig[number] | (string & {}) | false, command: () => Promise<T>, options?: Omit<UndefinedInitialDataOptions<T>, 'queryKey' | 'queryFn'>): UseQueryResult<T, Error> {
	return useQuery({
		queryKey: cacheKey === false ? [] : [cacheKey],
		queryFn: async () => {
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
		},
		enabled: true,
		...options,
	});
}
