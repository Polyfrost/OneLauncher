import type { Cluster, Loader, VersionType } from '~bindings';

export function formatVersionRelease(release: VersionType): string {
	const mapping: { [key in VersionType]: string } = {
		old_alpha: 'Alpha',
		old_beta: 'Beta',
		release: 'Release',
		snapshot: 'Snapshot',
	};

	return mapping[release];
}

export function supportsMods(loader: Cluster | Loader | undefined): boolean {
	if (loader === undefined)
		return false;

	if (typeof loader !== 'string')
		loader = loader.meta.loader;

	return loader !== 'vanilla';
}

/**
 * A simple analog of Node.js's `path.join(...)`.
 * https://gist.github.com/creationix/7435851#gistcomment-3698888
 */
export default function joinPath(...segments: string[]) {
	const parts = segments.reduce<string[]>((parts, segment) => {
		// Remove leading slashes from non-first part.
		if (parts.length > 0)
			segment = segment.replace(/^\//, '');

		// Remove trailing slashes.
		segment = segment.replace(/\/$/, '');
		return parts.concat(segment.split('/'));
	}, []);

	const resultParts: string[] = [];
	for (const part of parts) {
		if (part === '.')
			continue;

		if (part === '..') {
			resultParts.pop();
			continue;
		}
		resultParts.push(part);
	}
	return resultParts.join('/');
}
