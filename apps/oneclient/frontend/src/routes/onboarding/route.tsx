import type { DownloadModsRef } from '@/components';
import type { PropsWithChildren } from 'react';
import LauncherLogo from '@/assets/logos/oneclient.svg?react';
import { LoaderSuspense, NavbarButton } from '@/components';
import { GameBackground } from '@/components/GameBackground';
import { Stepper } from '@/components/Stepper';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute, Link, Outlet, useLocation, useNavigate } from '@tanstack/react-router';
import { Window } from '@tauri-apps/api/window';
import { MinusIcon, SquareIcon, XCloseIcon } from '@untitled-theme/icons-react';
import { motion } from 'motion/react';
import { useEffect } from 'react';
import { MouseParallax } from 'react-just-parallax';

export const Route = createFileRoute('/onboarding')({
	component: RouteComponent,
	loader: ({ location }) => {
		const currentStepIndex = LINEAR_ONBOARDING_STEPS.findIndex(
			step => step.path === location.pathname,
		);

		const isFirstStep = currentStepIndex === 0;
		const isLastStep = currentStepIndex === LINEAR_ONBOARDING_STEPS.length - 1;

		return {
			isFirstStep,
			isLastStep,
			previousPath:
				currentStepIndex > 0
					? LINEAR_ONBOARDING_STEPS[currentStepIndex - 1]?.path
					: undefined,
			nextPath:
				currentStepIndex < LINEAR_ONBOARDING_STEPS.length - 1
					? LINEAR_ONBOARDING_STEPS[currentStepIndex + 1]?.path
					: undefined,
			currentStepIndex,
		};
	},
});

export interface OnboardingStep {
	path: string;
	title: string;
	subSteps?: Array<OnboardingStep>;
	hideNavigationButtons?: boolean;
};

const ONBOARDING_STEPS: Array<OnboardingStep> = [
	{
		path: '/onboarding',
		title: 'Welcome',
	},
	{
		path: '/onboarding/language',
		title: 'Set Language',
	},
	{
		path: '/onboarding/account',
		title: 'Account',
		hideNavigationButtons: true,
	},
	{
		path: '/onboarding/preferences/',
		title: 'Preferences',
		subSteps: [
			{
				path: '/onboarding/preferences/version',
				title: 'Versions',
				hideNavigationButtons: true,
			},
			{
				path: '/onboarding/preferences/versionCategory',
				title: 'Versions Category',
				hideNavigationButtons: true,
			},
		],
	},
	{
		path: '/onboarding/finished',
		title: 'Finished',
	},
];

function getLinearSteps(steps: Array<OnboardingStep>): Array<Omit<OnboardingStep, 'subSteps'>> {
	return steps.flatMap(step => (step.subSteps ? step.subSteps : step));
}

const LINEAR_ONBOARDING_STEPS = getLinearSteps(ONBOARDING_STEPS);

function RouteComponent() {
	const location = useLocation();

	// Prefetch data so that onboarding/preferences/version is fast
	const queryClient = useQueryClient();
	const { data: clusters } = useCommandSuspense(['getClusters'], () => bindings.core.getClusters());
	useEffect(() => {
		clusters.forEach((cluster) => {
			queryClient.prefetchQuery({
				queryKey: ['getBundlesFor', cluster.id],
				queryFn: () => bindings.oneclient.getBundlesFor(cluster.id),
			});
		});
	}, [clusters, queryClient]);

	const { currentStepIndex } = Route.useLoaderData();

	return (
		// <LoaderSuspense spinner={{ size: 'large' }}>
		<AppShell>
			<div className="h-full w-full">
				<LoaderSuspense spinner={{ size: 'large' }}>
					<Navbar />

					<motion.div
						animate={{
							bottom: 0,
							opacity: 1,
						}}
						exit={{
							opacity: 0,
						}}
						initial={{
							opacity: 0,
						}}
						key={location.pathname}
						transition={{ duration: 0.25 }}
					>

						<Outlet />

					</motion.div>

					{LINEAR_ONBOARDING_STEPS[currentStepIndex].hideNavigationButtons ? <></> : <OnboardingNavigation />}
				</LoaderSuspense>
			</div>
		</AppShell>
		// </LoaderSuspense>
	);
}

function AppShell({
	children,
}: PropsWithChildren) {
	const { isFirstStep, currentStepIndex } = Route.useLoaderData();

	return (
		<div className="flex flex-col h-full w-full">
			<BackgroundGradient />

			<div className="h-screen flex overflow-hidden">
				<div className={`min-w-64 ${isFirstStep ? 'bg-page' : ''} border-r border-component-border flex flex-col`}>
					<div className="p-6">
						<div className="flex items-center gap-2">
							<LauncherLogo className="w-52 h-12" />
						</div>
					</div>

					<nav className="flex-1 p-4">
						<Stepper currentLinearIndex={currentStepIndex} linearSteps={LINEAR_ONBOARDING_STEPS} steps={ONBOARDING_STEPS} />
					</nav>

					<div className="p-4 text-xs text-fg-secondary">
						<p>version info</p>
					</div>
				</div>

				<div className={`flex-1 flex ${isFirstStep ? '' : 'bg-page'} flex-col relative`}>
					{children}
				</div>
			</div>
		</div>
	);
}

function BackgroundGradient() {
	// const background = useAppShellStore(state => state.background);

	// if (background === 'none')
	// 	return undefined;

	return (
		<div className="relative">
			{/* Linear black gradient: left -> right */}
			{/* <div
				className="absolute top-0 left-0 w-screen h-screen -z-10"
				style={{
					background: 'linear-gradient(270deg, rgba(0, 0, 0, 0.00) 35%, rgba(0, 0, 0, 0.60) 87.5%)',
				}}
			>
			</div> */}

			{/* Radial black gradient */}
			<div
				className="absolute top-0 left-0 w-screen h-screen -z-10" style={{
					background: 'radial-gradient(48.29% 48.29% at 77.29% 50%, rgba(0, 0, 0, 0.00) 0%, rgba(0, 0, 0, 0.64) 100%)',
				}}
			>
			</div>

			{/* Linear black gradient: bottom -> 200 px up */}
			<div
				className="absolute bottom-0 left-0 w-screen h-50 -z-10" style={{
					background: 'linear-gradient(180deg, rgba(17, 23, 28, 0.00) 0%, rgba(0, 0, 0, 0.68) 60%)',
				}}
			>
			</div>

			<MouseParallax isAbsolutelyPositioned strength={0.01} zIndex={-50}>
				<GameBackground
					className="absolute left-0 top-0 w-screen h-screen scale-110"
					name="MinecraftBuilding"
				/>
			</MouseParallax>
		</div>
	);
}

export function OnboardingNavigation({ ref, disableNext }: { ref?: React.RefObject<DownloadModsRef | null>; disableNext?: boolean }) {
	const navigate = useNavigate();
	const { isFirstStep, previousPath, nextPath } = Route.useLoaderData();

	function handleNextClick() {
		if (disableNext)
			return;

		if (ref && ref.current !== null)
			ref.current.openDownloadDialog(nextPath ?? '/app');
		else
			navigate({ to: nextPath ?? '/app' });
	}

	return (
		<div className="absolute bottom-2 right-2 flex flex-row gap-2">
			<div>
				{!isFirstStep && previousPath && (
					<Link to={previousPath}>
						<Button className="w-32" color="secondary">Back</Button>
					</Link>
				)}
			</div>
			<div>
				<Button
					className={`w-32 ${disableNext ? 'line-through' : ''}`}
					color={disableNext ? 'secondary' : 'primary'}
					isDisabled={disableNext}
					onClick={handleNextClick}
				>
					Next
				</Button>
			</div>
		</div>
	);
}

export function Navbar() {
	const onMinimize = () => Window.getCurrent().minimize();
	const onMaximize = () => Window.getCurrent().toggleMaximize();
	const onClose = () => Window.getCurrent().close();

	return (
		<nav className="flex flex-row items-center justify-between h-20 px-12 z-50" data-tauri-drag-region="true">
			<div className="flex flex-1 items-center justify-end gap-2 pointer-events-none">
				<NavbarButton
					children={<MinusIcon />}
					onClick={onMinimize}
				/>
				<NavbarButton
					children={<SquareIcon />}
					onClick={onMaximize}
				/>
				<NavbarButton
					children={(
						<XCloseIcon
							height={28}
							strokeWidth={1.5}
							width={28}
						/>
					)}
					className="bg-transparent"
					color="danger"
					onClick={onClose}
				/>
			</div>
		</nav>
	);
}
