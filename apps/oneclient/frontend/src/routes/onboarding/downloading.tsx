import type { ModData } from '@/components';
import { buildModDataArray, downloadModsParallel, isManagedMod } from '@/components';
import { bindings } from '@/main';
import useDownloadStore from '@/stores/downloadStore';
import { getCachedOnboardingTips, loadOnboardingTips } from '@/utils/onboardingFunFacts';
import { useToast } from '@/utils/toast';
import { getMessageFromError, isLauncherError, useCommandSuspense } from '@onelauncher/common';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

export const Route = createFileRoute('/onboarding/downloading')({
	component: RouteComponent,
});

// Navbar in onboarding is h-20 = 80px
const NAVBAR_HEIGHT = 80;
// How long the fade-out takes before navigating to /app
const FADE_DURATION_MS = 700;

interface DownloadFailure {
	mod: ModData;
	reason: string;
	index: number;
	clusterVersion: string;
}

function getErrorMessage(error: unknown): string {
	if (isLauncherError(error))
		return getMessageFromError(error);

	if (error instanceof Error)
		return error.message;

	if (typeof error === 'string')
		return error;

	try {
		return JSON.stringify(error);
	}
	catch {
		return 'Unknown download error';
	}
}

function RouteComponent() {
	const navigate = useNavigate();
	const queryClient = useQueryClient();
	const { modsPerCluster, isFinishingOnboarding } = useDownloadStore();
	const clearStore = useDownloadStore(s => s.clear);
	const markOnboardingFinishing = useDownloadStore(s => s.markOnboardingFinishing);
	const toast = useToast();
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());
	const { data: settings } = useCommandSuspense(['readSettings'], bindings.core.readSettings);
	const [tips, setTips] = useState<Array<string>>(getCachedOnboardingTips);

	const clusterVersionById = useMemo(() => {
		return new Map(clusters.map(cluster => [cluster.id, cluster.mc_version]));
	}, [clusters]);

	const tip = useMemo(() => tips[Math.floor(Math.random() * tips.length)], [tips]);

	useEffect(() => {
		let cancelled = false;

		const loadTips = async () => {
			try {
				const loadedTips = await loadOnboardingTips();
				if (!cancelled && loadedTips.length > 0)
					setTips(loadedTips);
			}
			catch (error) {
				console.warn('[onboarding/downloading] Falling back to backup fun facts.', error);
			}
		};

		void loadTips();

		return () => {
			cancelled = true;
		};
	}, []);

	// Only redirect on initial mount if store was already empty (safety net for direct navigation).
	// Must NOT re-run when clearStore() empties the store at the end of downloads.
	const initiallyEmpty = useRef(Object.keys(modsPerCluster).length === 0);

	useEffect(() => {
		if (!initiallyEmpty.current)
			return;

		if (isFinishingOnboarding)
			return;

		navigate({ to: '/onboarding/preferences/versionCategory' });
	}, [isFinishingOnboarding, navigate]);

	const mods = useMemo(() => buildModDataArray(modsPerCluster), [modsPerCluster]);
	const onboardingInstalledVersions = useMemo(() => {
		const versions = new Set(clusters.map(cluster => cluster.mc_version));
		return [...versions].sort();
	}, [clusters]);

	const [processedMods, setProcessedMods] = useState(0);
	const [successfulMods, setSuccessfulMods] = useState(0);
	const [failedMods, setFailedMods] = useState<Array<DownloadFailure>>([]);
	const [batchError, setBatchError] = useState<string | null>(null);
	const [retryQueue, setRetryQueue] = useState<Array<ModData>>([]);
	const [currentBatchTotal, setCurrentBatchTotal] = useState(0);
	const [isRunningBatch, setIsRunningBatch] = useState(false);
	const [isComplete, setIsComplete] = useState(false);
	const [isLeaving, setIsLeaving] = useState(false);
	const hasStarted = useRef(false);
	const finishTimerRef = useRef<number | null>(null);

	const percentage = currentBatchTotal > 0 ? Math.round((processedMods / currentBatchTotal) * 100) : 0;
	const finishOnboarding = useCallback(() => {
		if (isLeaving)
			return;

		markOnboardingFinishing();

		setIsLeaving(true);

		if (finishTimerRef.current !== null)
			window.clearTimeout(finishTimerRef.current);

		finishTimerRef.current = window.setTimeout(() => {
			void (async () => {
				try {
					const storedSeenVersions = Array.isArray(settings.seen_versions) ? settings.seen_versions : [];
					const nextSeenVersions = [
						...new Set([...onboardingInstalledVersions, ...storedSeenVersions]),
					].sort();
					const nextSettings = {
						...settings,
						seen_onboarding: true,
						seen_versions: nextSeenVersions,
					};

					await bindings.core.writeSettings(nextSettings);
					queryClient.setQueryData(['readSettings'], nextSettings);
				}
				catch (error) {
					console.error('[onboarding/downloading] Failed to persist seen_onboarding before continue; navigating anyway.', error);
				}

				navigate({ to: '/app', replace: true });
				clearStore();
			})();
		}, FADE_DURATION_MS);
	}, [
		clearStore,
		isLeaving,
		markOnboardingFinishing,
		navigate,
		onboardingInstalledVersions,
		queryClient,
		settings,
	]);

	useEffect(() => {
		return () => {
			if (finishTimerRef.current !== null)
				window.clearTimeout(finishTimerRef.current);
		};
	}, []);

	const downloadMod = async (mod: ModData): Promise<void> => {
		if (isManagedMod(mod) && mod.bundleName) {
			await bindings.oneclient.downloadPackageFromBundle(mod.fileKind, mod.clusterId, mod.bundleName, true);
			return;
		}

		if (isManagedMod(mod)) {
			await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
			return;
		}

		if (mod.bundleName)
			await bindings.oneclient.downloadPackageFromBundle(mod.fileKind, mod.clusterId, mod.bundleName, true);
		else
			await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null);
	};

	const startDownloadBatch = async (batchMods: Array<ModData>) => {
		setIsRunningBatch(true);
		setBatchError(null);
		setFailedMods([]);
		setRetryQueue([]);
		setIsComplete(false);
		setProcessedMods(0);
		setSuccessfulMods(0);
		setCurrentBatchTotal(batchMods.length);

		try {
			const results = await downloadModsParallel(batchMods, 10, async (mod) => {
				try {
					await downloadMod(mod);
					setSuccessfulMods(prev => prev + 1);
				}
				finally {
					setProcessedMods(prev => prev + 1);
				}
			});

			const failures = results
				.filter(result => !result.ok)
				.map((result) => {
					return {
						mod: result.mod,
						reason: getErrorMessage(result.error),
						index: result.index,
						clusterVersion: clusterVersionById.get(result.mod.clusterId) ?? 'Unknown version',
					};
				});

			if (failures.length > 0) {
				setFailedMods(failures);
				setRetryQueue(failures.map(failure => failure.mod));
				toast({
					type: 'error',
					title: 'Some mods failed to download',
					message: `${failures.length} of ${batchMods.length} failed. Review the summary and retry.`,
				});
			}
		}
		catch (error) {
			const message = getErrorMessage(error);
			setBatchError(message);
			setRetryQueue(batchMods);
			toast({
				type: 'error',
				title: 'Download batch crashed',
				message,
			});
		}
		finally {
			setIsRunningBatch(false);
			setIsComplete(true);
		}
	};

	useEffect(() => {
		if (hasStarted.current || mods.length === 0)
			return;
		hasStarted.current = true;
		void startDownloadBatch(mods);
	// eslint-disable-next-line react-hooks/exhaustive-deps -- startDownloadBatch closes over mutable progress state; this effect intentionally runs once per mods snapshot.
	}, [mods]);

	useEffect(() => {
		if (!isComplete || failedMods.length > 0 || batchError !== null)
			return;

		// Brief pause at 100%, then fade out, then navigate straight to /app.
		const pauseTimer = setTimeout(() => {
			finishOnboarding();
		}, 400);

		return () => clearTimeout(pauseTimer);
	}, [batchError, failedMods.length, finishOnboarding, isComplete]);

	const handleRetryFailed = () => {
		if (retryQueue.length === 0 || isRunningBatch)
			return;

		void startDownloadBatch(retryQueue);
	};

	const hasFailures = failedMods.length > 0 || batchError !== null;
	const heading = hasFailures ? 'Download issues found' : 'Downloading...';

	return (
		<div
			className="flex flex-col px-12 pb-12 transition-opacity"
			style={{
				// min-height fills the screen below the navbar without relying on h-full,
				// which collapses inside the motion.div wrapper that has no explicit height.
				minHeight: `calc(100vh - ${NAVBAR_HEIGHT}px)`,
				paddingTop: '1rem',
				opacity: isLeaving ? 0 : 1,
				transitionDuration: `${FADE_DURATION_MS}ms`,
				transitionTimingFunction: 'ease-in-out',
			}}
		>
			{/* Top left: title */}
			<h1 className="text-4xl font-bold text-fg-primary">
				{heading}
			</h1>

			{hasFailures && isComplete
				? (
						<div className="mt-6 max-w-2xl rounded-lg border border-red-500/50 bg-red-500/10 p-5 text-fg-primary">
							<p className="text-lg font-semibold">Some mods could not be downloaded.</p>
							<p className="mt-2 text-sm text-fg-secondary">
								Successful: {successfulMods} / {currentBatchTotal}
							</p>
							{batchError
								? <p className="mt-3 text-sm text-red-200">Batch error: {batchError}</p>
								: null}
							{failedMods.length > 0
								? (
										<div className="mt-4 max-h-40 overflow-auto rounded bg-black/20 p-3 text-sm">
											{failedMods.slice(0, 8).map((failure) => {
												return (
													<p key={`${failure.mod.clusterId}-${failure.mod.name}-${failure.index}`}>
														{failure.mod.name} (MC {failure.clusterVersion}, cluster {failure.mod.clusterId}): {failure.reason}
													</p>
												);
											})}
											{failedMods.length > 8
												? <p className="mt-2 text-fg-secondary">+{failedMods.length - 8} more failures</p>
												: null}
										</div>
									)
								: null}
							<div className="mt-4 flex gap-3">
								<button
									className="rounded bg-white/90 px-4 py-2 text-black transition hover:bg-white"
									onClick={handleRetryFailed}
									type="button"
								>
									Retry Failed Mods
								</button>
								<button
									className="rounded border border-white/30 px-4 py-2 text-white transition hover:bg-white/10"
									onClick={finishOnboarding}
									type="button"
								>
									Continue Anyway
								</button>
							</div>
						</div>
					)
				: null}

			{/* Spacer — pushes bottom content to the bottom */}
			<div className="flex-1" />

			{/* Bottom row: percentage+bar on left, tip on right */}
			<div className="flex items-end justify-between gap-12">
				{/* Left: percentage + progress bar */}
				<div className="flex flex-col gap-2 w-64">
					<span className="text-2xl font-semibold text-fg-primary">
						{percentage}
						%
					</span>
					<span className="text-xs text-fg-secondary">
						Processed: {processedMods} / {currentBatchTotal}
					</span>
					<div className="w-full h-0.5 rounded-full bg-white/20">
						<div
							className="h-full rounded-full bg-white transition-all duration-300"
							style={{ width: `${percentage}%` }}
						/>
					</div>
				</div>

				{/* Right: tip */}
				<div className="max-w-xs text-right">
					<p className="text-xs font-semibold uppercase tracking-widest text-fg-secondary mb-1">Tip</p>
					<p className="text-sm text-fg-secondary whitespace-pre-line">{tip}</p>
				</div>
			</div>
		</div>
	);
}
