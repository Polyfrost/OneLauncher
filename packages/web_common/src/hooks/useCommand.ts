import type { UndefinedInitialDataOptions, UseQueryResult } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';

type CacheKey = string;

export function useCommand<T>(cacheKey: CacheKey | (string & {}), command: () => Promise<T>, options?: Omit<UndefinedInitialDataOptions<T>, 'queryKey' | 'queryFn'>): UseQueryResult<T, Error> {
	return useQuery({
		queryKey: [cacheKey],
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

// export useCommand;
