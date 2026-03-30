import type { ModData } from '@/components';
import { buildModDataArray, downloadModsParallel, isManagedMod } from '@/components';
import { useSettings } from '@/hooks/useSettings';
import { bindings } from '@/main';
import useDownloadStore from '@/stores/downloadStore';
import { getCachedOnboardingTips, loadOnboardingTips } from '@/utils/onboardingFunFacts';
import { useCommandMut } from '@onelauncher/common';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useEffect, useMemo, useRef, useState } from 'react';

export const Route = createFileRoute('/onboarding/downloading')({
	component: RouteComponent,
});

// Navbar in onboarding is h-20 = 80px
const NAVBAR_HEIGHT = 80;
// How long the fade-out takes before navigating to /app
const FADE_DURATION_MS = 700;

function RouteComponent() {
	const navigate = useNavigate();
	const { modsPerCluster } = useDownloadStore();
	const clearStore = useDownloadStore(s => s.clear);
	const { setSetting } = useSettings();
	const [tips, setTips] = useState<Array<string>>(getCachedOnboardingTips);

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
		if (initiallyEmpty.current)
			navigate({ to: '/onboarding/preferences/versionCategory' });
	}, [navigate]);

	const mods = useMemo(() => buildModDataArray(modsPerCluster), [modsPerCluster]);

	const [downloadedMods, setDownloadedMods] = useState(0);
	const [isComplete, setIsComplete] = useState(false);
	const [isLeaving, setIsLeaving] = useState(false);
	const hasStarted = useRef(false);

	const percentage = mods.length > 0 ? Math.round((downloadedMods / mods.length) * 100) : 0;

	const download = useCommandMut(async (mod: ModData) => {
		if (isManagedMod(mod)) {
			if (mod.dependencies.length > 0)
				for (const dependency of mod.dependencies) {
					const cluster = await bindings.core.getClusterById(mod.clusterId);
					if (!cluster)
						continue;
					if (dependency.dependency_type === 'required') {
						const slug = dependency.project_id ?? '';
						const versions = await bindings.core.getPackageVersions(mod.provider, slug, cluster.mc_version, cluster.mc_loader, 0, 1);
						if (versions.items.length !== 0)
							await bindings.core.downloadPackage(mod.provider, slug, versions.items[0].version_id, cluster.id, null);
					}
				}

			if (mod.bundleName)
				await bindings.oneclient.downloadPackageFromBundle(mod.fileKind, mod.clusterId, mod.bundleName, true);
			else
				await bindings.core.downloadPackage(mod.provider, mod.id, mod.versionId, mod.clusterId, true);
		}
		else {
			if (mod.bundleName)
				await bindings.oneclient.downloadPackageFromBundle(mod.fileKind, mod.clusterId, mod.bundleName, true);
			else
				await bindings.core.downloadExternalPackage(mod.package, mod.clusterId, null, null);
		}
	});

	useEffect(() => {
		if (hasStarted.current || mods.length === 0)
			return;
		hasStarted.current = true;

		const downloadAll = async () => {
			await downloadModsParallel(mods, 10, async (mod) => {
				try {
					await download.mutateAsync(mod);
				}
				catch (e) {
					// Log the full Tauri error object so we can see what went wrong
					console.error(
						`[OneClient] Failed to download mod "${mod.name}" (cluster ${mod.clusterId}):`,
						JSON.stringify(e),
						e,
					);
				}
				finally {
					// Always increment even on error so progress still moves forward
					setDownloadedMods(prev => prev + 1);
				}
			});
			setIsComplete(true);
		};

		// Catch-all: if downloadAll itself rejects for any unexpected reason,
		// still mark complete so the user isn't stuck on this screen.
		downloadAll().catch((e) => {
			console.error('[OneClient] Unexpected error in downloadAll, forcing completion:', JSON.stringify(e), e);
			setIsComplete(true);
		});
	// eslint-disable-next-line react-hooks/exhaustive-deps -- download is not stable
	}, [mods]);

	useEffect(() => {
		if (!isComplete)
			return;

		// Brief pause at 100%, then fade out, then navigate straight to /app.
		const pauseTimer = setTimeout(() => {
			setIsLeaving(true);

			const fadeTimer = setTimeout(() => {
				// Mark onboarding as seen (previously done by finished.tsx)
				setSetting('seen_onboarding', true);
				// Navigate first, then clear the store to avoid the empty-store redirect firing
				navigate({ to: '/app' });
				clearStore();
			}, FADE_DURATION_MS);

			return () => clearTimeout(fadeTimer);
		}, 400);

		return () => clearTimeout(pauseTimer);
	}, [isComplete, clearStore, navigate, setSetting]);

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
				Downloading...
			</h1>

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
