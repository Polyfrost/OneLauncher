import type { FileRouteTypes } from '@/routeTree.gen';

const mapping: Partial<Record<FileRouteTypes['to'], string>> = {
	'/app': 'Home',
	'/app/clusters': 'Clusters',
};

export function getLocationName(
	to: FileRouteTypes['to'] | (string & {}) | undefined | null,
): string | undefined {
	if (!to)
		return undefined;

	if (to in mapping)
		return mapping[to as keyof typeof mapping];

	const mostSimilarPath = getMostSimilarPathMapping(to);
	return mostSimilarPath;
}

function getMostSimilarPathMapping(
	to: string,
): FileRouteTypes['to'] | undefined {
	let path = to;

	while (path !== '') {
		if (Object.prototype.hasOwnProperty.call(mapping, path))
			return path as FileRouteTypes['to'];

		const lastSlash = path.lastIndexOf('/');

		if (lastSlash === 0)
			return Object.prototype.hasOwnProperty.call(mapping, '/') ? '/' : undefined;

		path = path.substring(0, lastSlash);
	}

	return undefined;
}
