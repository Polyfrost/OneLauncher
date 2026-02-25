import { bindings } from '@/main';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

// Track which paths have already been refreshed this session so we only
// hit the network once per path, not on every component mount.
const refreshedPaths: Set<string> = new Set();

/**
 * Returns the best URL for an art image at the given relative path (e.g. `/versions/art/Foo.png`).
 *
 * - If the image is already cached on disk, returns the local `asset://` URL immediately.
 * - If not cached, downloads it first, then returns the local URL.
 * - After the cached version is shown, triggers a one-per-session background refresh that
 *   overwrites the on-disk cache with any updated version — applied on the next app start.
 */
export function useCachedImage(path: string | null | undefined): string | undefined {
	const [src, setSrc] = useState<string | undefined>(undefined);

	useEffect(() => {
		setSrc(undefined);
		if (!path)
			return;

		let cancelled = false;

		bindings.oneclient.cacheArt(path)
			.then((localPath: string) => {
				if (cancelled)
					return;

				setSrc(convertFileSrc(localPath));

				// One background refresh per path per session — no UI update, just overwrites disk
				if (!refreshedPaths.has(path)) {
					refreshedPaths.add(path);
					bindings.oneclient.refreshArt(path).catch(() => {});
				}
			})
			.catch(() => {
				// Not cached and offline — nothing to show
			});

		return () => {
			cancelled = true;
		};
	}, [path]);

	return src;
}
