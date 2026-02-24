import type { DebouncedWatchOptions, WatchEvent } from '@tauri-apps/plugin-fs';
import { useLastPlayedClusters } from '@/hooks/useClusters';
import { bindings } from '@/main';
import { useQueryClient } from '@tanstack/react-query';
import { invoke, SERIALIZE_TO_IPC_FN, transformCallback } from '@tauri-apps/api/core';
import { useEffect, useMemo } from 'react';

const WATCH_DELAY_MS = 1000;
const SYNC_DEBOUNCE_MS = 1000;
const STALE_WATCH_CALLBACK_TTL_MS = 30_000;

interface WatchTarget {
	id: number;
	folderName: string;
}

interface ChannelMessage<T> {
	message: T;
	index: number;
}

interface ChannelEndMessage {
	end: true;
	index: number;
}

type RawChannelMessage<T> = ChannelMessage<T> | ChannelEndMessage;

class WatchChannel<T> {
	private readonly callbackId: number;
	private disposed = false;

	constructor(onMessage: (message: T) => void) {
		this.callbackId = transformCallback((rawMessage: RawChannelMessage<T>) => {
			if (this.disposed)
				return;

			if (!('message' in rawMessage))
				return;

			onMessage(rawMessage.message);
		}, false);
	}

	[SERIALIZE_TO_IPC_FN]() {
		return `__CHANNEL__:${this.callbackId}`;
	}

	toJSON() {
		return this[SERIALIZE_TO_IPC_FN]();
	}

	scheduleCleanup() {
		const callbackKey = `_${this.callbackId}`;
		const callbacks = window as unknown as Record<string, unknown>;
		this.disposed = true;

		// Keep a noop callback briefly after unwatch to absorb late native events.
		callbacks[callbackKey] = () => {};
		window.setTimeout(() => {
			Reflect.deleteProperty(window, callbackKey);
		}, STALE_WATCH_CALLBACK_TTL_MS);
	}
}

async function watchSafely(
	path: string,
	callback: (event: WatchEvent) => void,
	options: DebouncedWatchOptions,
) {
	const onEvent: WatchChannel<WatchEvent> = new WatchChannel(callback);
	const rid = await invoke<number>('plugin:fs|watch', {
		paths: [path],
		options: {
			recursive: false,
			delayMs: 2000,
			...options,
		},
		onEvent,
	});

	let closed = false;

	return () => {
		if (closed)
			return;

		closed = true;
		void invoke('plugin:fs|unwatch', { rid }).finally(() => {
			onEvent.scheduleCleanup();
		});
	};
}

function normalizePath(path: string): string {
	return path.replaceAll('\\', '/').toLowerCase();
}

function isStructuralChange(event: WatchEvent): boolean {
	if (typeof event.type === 'string')
		return false;

	return 'create' in event.type || 'remove' in event.type;
}

function shouldIgnoreEvent(event: WatchEvent): boolean {
	// Ignore plain file edits (modify/metadata/data writes). Sync only on
	// structural changes (add/remove file or folder).
	if (!isStructuralChange(event))
		return true;

	if (event.paths.length === 0)
		return false;

	return event.paths.every((rawPath) => {
		const path = normalizePath(rawPath);
		return path.includes('/logs/')
			|| path.includes('/crash-reports/')
			|| path.includes('/saves/')
			|| path.endsWith('/latest.log');
	});
}

/**
 * Watches ALL cluster directories for file-system changes (files added,
 * removed, renamed). Should be mounted once at the app root so that syncing
 * happens regardless of which page the user is on.
 */
export function useAllClusterDirWatch() {
	const { data: clusters } = useLastPlayedClusters();
	const queryClient = useQueryClient();
	const watchTargets: Array<WatchTarget> = useMemo(
		() => clusters.map(cluster => ({
			id: cluster.id,
			folderName: cluster.folder_name,
		})),
		[clusters],
	);

	useEffect(() => {
		if (watchTargets.length === 0)
			return;

		const lifecycle = {
			disposed: false,
		};
		const unwatchers: Array<() => void> = [];
		const pendingTimeouts: Map<number, number> = new Map();
		const syncInFlight: Set<number> = new Set();
		const syncQueued: Set<number> = new Set();

		const clearPendingSync = (clusterId: number) => {
			const timeoutId = pendingTimeouts.get(clusterId);
			if (timeoutId === undefined)
				return;

			window.clearTimeout(timeoutId);
			pendingTimeouts.delete(clusterId);
		};

		const runSync = async (clusterId: number) => {
			if (lifecycle.disposed)
				return;

			if (syncInFlight.has(clusterId)) {
				syncQueued.add(clusterId);
				return;
			}

			syncInFlight.add(clusterId);
			try {
				// Skip sync while the backend is applying bundle updates to avoid
				// race conditions (watcher would re-sync mid-download).
				const bundleSyncing = await bindings.oneclient.isBundleSyncing().catch(() => false);
				if (bundleSyncing)
					return;

				// Skip sync while cluster is running to avoid constant scans while
				// the game is actively writing files.
				const running = await bindings.core.isClusterRunning(clusterId).catch(() => false);
				if (running)
					return;

				await bindings.core.syncCluster(clusterId);
				await queryClient.invalidateQueries({ queryKey: ['getLinkedPackages', clusterId] });
			}
			catch (e) {
				console.error('[watch] sync failed for cluster', clusterId, ':', e);
			}
			finally {
				syncInFlight.delete(clusterId);
				if (syncQueued.has(clusterId)) {
					syncQueued.delete(clusterId);
					scheduleSync(clusterId);
				}
			}
		};

		const scheduleSync = (clusterId: number) => {
			if (lifecycle.disposed)
				return;

			clearPendingSync(clusterId);
			const timeoutId = window.setTimeout(() => {
				pendingTimeouts.delete(clusterId);
				void runSync(clusterId);
			}, SYNC_DEBOUNCE_MS);
			pendingTimeouts.set(clusterId, timeoutId);
		};

		const isDisposed = () => lifecycle.disposed;

		const registerWatch = async (target: WatchTarget) => {
			try {
				const clusterDir = await bindings.folders.fromCluster(target.folderName);
				if (isDisposed())
					return;

				const unwatch = await watchSafely(
					clusterDir,
					(event) => {
						if (isDisposed() || shouldIgnoreEvent(event))
							return;

						scheduleSync(target.id);
					},
					{ delayMs: WATCH_DELAY_MS, recursive: true },
				);

				if (isDisposed()) {
					unwatch();
					return;
				}

				unwatchers.push(unwatch);
			}
			catch (e) {
				console.error('[watch] failed to register watch:', e);
			}
		};

		for (const target of watchTargets)
			void registerWatch(target);

		return () => {
			lifecycle.disposed = true;
			for (const timeoutId of pendingTimeouts.values())
				window.clearTimeout(timeoutId);
			pendingTimeouts.clear();
			syncQueued.clear();
			syncInFlight.clear();

			for (const unwatch of unwatchers)
				unwatch();
		};
	}, [queryClient, watchTargets]);
}
