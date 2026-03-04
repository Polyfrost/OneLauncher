import { bindings } from '@/main';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';

// Track which paths have already been refreshed this session so we only
// hit the network once per path, not on every component mount.
const refreshedPaths: Set<string> = new Set();
const prefetchedPaths: Set<string> = new Set();

function normalizePaths(paths: Array<string | null | undefined>): Array<string> {
	return Array.from(new Set(paths.filter((path): path is string => Boolean(path))));
}

/**
 * Eagerly caches a list of art paths with bounded concurrency.
 * This is intended for onboarding prefetch so version cards can render
 * immediately from disk cache instead of waiting for per-card fetches.
 */
export async function prefetchCachedImages(
	paths: Array<string | null | undefined>,
	options?: { concurrency?: number },
): Promise<void> {
	const normalizedPaths = normalizePaths(paths).filter(path => !prefetchedPaths.has(path));
	if (normalizedPaths.length === 0)
		return;

	const concurrency = Math.max(1, options?.concurrency ?? 4);
	let nextIndex = 0;

	const worker = async () => {
		while (nextIndex < normalizedPaths.length) {
			const index = nextIndex++;
			const path = normalizedPaths[index];
			if (!path)
				continue;

			prefetchedPaths.add(path);
			try {
				await bindings.oneclient.cacheArt(path);
				if (!refreshedPaths.has(path)) {
					refreshedPaths.add(path);
					bindings.oneclient.refreshArt(path).catch(() => {});
				}
			}
			catch {
				// Keep failures non-fatal and allow retry in a future prefetch call.
				prefetchedPaths.delete(path);
			}
		}
	};

	await Promise.all(Array.from({ length: Math.min(concurrency, normalizedPaths.length) }, worker));
}

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
