import type { bindings } from '@/main';
import type { UndefinedInitialDataOptions } from '@tanstack/react-query';
import { useQuery } from '@tanstack/react-query';

type CacheKey = (keyof typeof bindings.core) | (keyof typeof bindings.onelauncher);

function useCommand<T>(cacheKey: CacheKey | (string & {}), command: () => Promise<T>, options?: Omit<UndefinedInitialDataOptions<T>, 'queryKey' | 'queryFn'>) {
	return useQuery({
		queryKey: [cacheKey],
		queryFn: async () => {
			try {
				const result = await command();
				return result;
			}
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

export default useCommand;
