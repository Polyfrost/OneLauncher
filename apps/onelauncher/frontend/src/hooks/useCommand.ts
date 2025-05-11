import type { Result } from '@/bindings.gen';
import type { bindings } from '@/main';
import type { UndefinedInitialDataOptions } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';

type CacheKey = keyof typeof bindings.commands;

function useCommand<T, TError>(cacheKey: CacheKey | (string & {}), command: () => Promise<Result<T, TError>>, options?: Omit<UndefinedInitialDataOptions<T>, 'queryKey' | 'queryFn'>) {
	return useQuery({
		queryKey: [cacheKey],
		queryFn: async () => {
			try {
				const result = await command();
				if (result.status === 'error')
					return Promise.reject(result.error);

				return result.data;
			}
			catch (e) {
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

export default useCommand;
