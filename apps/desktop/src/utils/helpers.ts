import type { VersionType } from '~bindings';

export function formatVersionRelease(release: VersionType): string {
	const mapping: { [key in VersionType]: string } = {
		old_alpha: 'Alpha',
		old_beta: 'Beta',
		release: 'Release',
		snapshot: 'Snapshot',
	};

	return mapping[release];
}
