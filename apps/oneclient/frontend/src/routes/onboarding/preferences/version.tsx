import type { GameLoader, OnlineCluster, OnlineClusterEntry } from '@/bindings.gen';
import { useCachedImage } from '@/hooks/useCachedImage';
import { bindings } from '@/main';
import { OnboardingNavigation } from '@/routes/onboarding/route';
import { formatMcVersion } from '@/utils/versionMap';
import { useCommandSuspense } from '@onelauncher/common';
import { createFileRoute } from '@tanstack/react-router';
import { ArrowLeftIcon, ArrowRightIcon } from '@untitled-theme/icons-react';
import { useEffect, useMemo, useState } from 'react';
import { Button as AriaButton } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';

export interface StrippedCluster {
	mc_version: string;
	mc_loader: GameLoader;
}

interface VersionSlide {
	cluster: OnlineCluster;
	entry: OnlineClusterEntry;
	fullVersionName: string;
}

export const Route = createFileRoute('/onboarding/preferences/version')({
	component: RouteComponent,
});

function RouteComponent() {
	const { data: versions } = useCommandSuspense(['getVersions'], () => bindings.oneclient.getVersions());

	const slides = useMemo<Array<VersionSlide>>(() => {
		return versions.clusters.flatMap(cluster =>
			cluster.entries.map(entry => ({
				cluster,
				entry,
				fullVersionName: formatMcVersion(cluster.major_version, entry.minor_version),
			})));
	}, [versions.clusters]);

	useEffect(() => {
		const allClusters: Array<StrippedCluster> = slides.map(s => ({
			mc_version: s.fullVersionName,
			mc_loader: s.entry.loader,
		}));
		localStorage.setItem('selectedClusters', JSON.stringify(allClusters));
	}, [slides]);

	const [currentIndex, setCurrentIndex] = useState(0);

	const goLeft = () => setCurrentIndex(i => Math.max(0, i - 1));
	const goRight = () => setCurrentIndex(i => Math.min(slides.length - 1, i + 1));

	if (slides.length === 0)
		return <OnboardingNavigation />;

	const current = slides[currentIndex];

	return (
		<>
			<div className="min-h-screen px-7">
				<div className="max-w-6xl mx-auto">
					<div className="h-[calc(100vh-12rem)] flex flex-col">
						<h1 className="text-4xl font-semibold mb-2">Available Versions</h1>
						<p className="text-slate-400 text-lg mb-4">
							These are the versions OneClient will set up for you. Browse through to see what's available.
						</p>

						<div className="flex-1 min-h-0 flex flex-col items-center gap-4">
							<div className="relative w-full max-w-4xl flex-1 min-h-0">
								<CarouselSlide
									className="h-full"
									cluster={current.cluster}
									fullVersionName={current.fullVersionName}
									version={current.entry}
								/>

								<AriaButton
									className={twMerge(
										'absolute left-3 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-2 transition-colors',
										currentIndex === 0 && 'opacity-30 cursor-default',
									)}
									isDisabled={currentIndex === 0}
									onPress={goLeft}
								>
									<ArrowLeftIcon className="w-5 h-5" />
								</AriaButton>

								<AriaButton
									className={twMerge(
										'absolute right-3 top-1/2 -translate-y-1/2 bg-black/50 hover:bg-black/70 text-white rounded-full p-2 transition-colors',
										currentIndex === slides.length - 1 && 'opacity-30 cursor-default',
									)}
									isDisabled={currentIndex === slides.length - 1}
									onPress={goRight}
								>
									<ArrowRightIcon className="w-5 h-5" />
								</AriaButton>
							</div>

							<div className="flex items-center gap-2">
								{slides.map((slide, i) => (
									<AriaButton
										className={twMerge(
											'w-2.5 h-2.5 rounded-full transition-all',
											i === currentIndex ? 'bg-brand scale-125' : 'bg-white/30 hover:bg-white/50',
										)}
										key={`${slide.fullVersionName}-${slide.entry.loader}`}
										onPress={() => setCurrentIndex(i)}
									/>
								))}
							</div>

							<span className="text-sm text-slate-400">
								{currentIndex + 1}
								{' '}
								/
								{' '}
								{slides.length}
							</span>
						</div>
					</div>
				</div>
			</div>

			<OnboardingNavigation />
		</>
	);
}

function CarouselSlide({ cluster, version, fullVersionName, className }: { cluster: OnlineCluster; version: OnlineClusterEntry; fullVersionName: string; className?: string }) {
	const artPath = version.art ?? cluster.art;
	const artSrc = useCachedImage(artPath);

	return (
		<div className={twMerge('overflow-hidden rounded-xl', className)}>
			<div className="relative w-full h-full">
				{artSrc
					? (
							<img
								alt={`Minecraft ${fullVersionName} landscape`}
								className="w-full rounded-xl h-full object-cover"
								src={artSrc}
							/>
						)
					: (
							<div className="w-full rounded-xl h-full bg-page-elevated" />
						)}

				<div className="absolute top-3 left-3 flex flex-wrap gap-1">
					{version.tags.map(tag => (
						<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1" key={tag}>
							{tag}
						</div>
					))}
				</div>

				<div className="absolute bottom-3 left-3">
					<span className="text-white font-bold px-4 py-2 text-4xl drop-shadow-lg">{fullVersionName}</span>
				</div>
			</div>
		</div>
	);
}
