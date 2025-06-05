import type { JSX } from 'react';
import { Button } from '@onelauncher/common/components';
import { createFileRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';

export const Route = createFileRoute('/onboarding')({
	component: RouteComponent,
});

const steps = [
	'/onboarding', // done
	'/onboarding/language', // done (only ui)
	'/onboarding/login', // wip
	'/onboarding/import', // wip

	'/onboarding/summary', // wip
	'/onboarding/complete', // wip
] as const;

interface Language {
	lang: string;
	percentage: number;
}

interface ImportInstancesType {
	basePath: string;
	instances: Array<string>;
}

interface _OnboardingContextType {
	setLanguage: (language: Language) => void;
	language: () => Language;

	setImportInstances: (type: string, basePath: string, instances: Array<string>) => void;
	importInstances: (type: string) => ImportInstancesType | undefined;

	setForwardButtonEnabled: (enabled: boolean) => void;

	getTasks: () => Array<string>;
}

function RouteComponent() {
	const navigate = useNavigate();
	const routerState = useRouterState();

	const currentPath = routerState.location.pathname;
	const currentStepIndex = steps.findIndex(path => currentPath === path || currentPath.startsWith(path));

	const progressPercentage = currentStepIndex >= 0
		? (currentStepIndex / (steps.length - 1)) * 100
		: 0;

	const handleBack = () => {
		if (currentStepIndex > 0) {
			const previousStep = steps[currentStepIndex - 1];
			navigate({ to: previousStep as any });
		}
	};

	const handleNext = () => {
		if (currentStepIndex === -1) {
			navigate({ to: steps[0] as any });
			return;
		}

		if (currentStepIndex < steps.length - 1) {
			const nextStep = steps[currentStepIndex + 1];
			navigate({ to: nextStep as any });
		}
		else if (currentStepIndex === steps.length - 1) {
			navigate({ to: '/app' as any });
		}
	};

	return (
	// remind me 2 hours! i'll fix this
	// update: it's fixed!
		<div className="w-full flex flex-col items-center h-screen">
			<div className="h-0.5 w-full">
				<div
					className="h-full rounded-lg bg-brand transition-all"
					style={{
						width: `${progressPercentage}%`,
					}}
				/>
			</div>

			<div className="flex-1 max-w-280 w-full flex flex-col gap-y-4 p-8">
				<Outlet />
			</div>

			<div className="w-full max-w-280 p-8">
				<div className="w-1/3 flex flex-row items-stretch gap-x-8 [&>*]:w-full ml-auto">
					<Button onClick={handleBack}>Back</Button>
					<Button onClick={handleNext}>{currentStepIndex === steps.length - 1 ? 'Finish' : 'Next'}</Button>
				</div>
			</div>
		</div>
	);
}

export interface OnboardingStepProps {
	title: string;
	paragraph: string;
	illustration: JSX.Element;
	children: JSX.Element;
}

export function OnboardingStep(props: OnboardingStepProps) {
	const { illustration, title, paragraph, children } = props;

	return (
		<div className="grid grid-cols-2 h-full w-full gap-x-16">
			<div className="flex flex-col items-center justify-center">
				{illustration}
			</div>

			<div className="flex flex-col justify-center gap-y-4">
				<div className="w-full flex flex-col gap-y-2">
					<h1 className="text-2xl">{title}</h1>
					<p className="text-lg text-fg-secondary line-height-normal">{paragraph}</p>
				</div>

				<div className="max-h-96 w-full flex flex-1 flex-col gap-y-2">
					{children}
				</div>
			</div>
		</div>
	);
}
