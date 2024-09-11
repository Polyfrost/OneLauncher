import { Route, useLocation, useNavigate } from '@solidjs/router';
import { ChevronLeftIcon, ChevronRightIcon } from '@untitled-theme/icons-solid';
import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import Button from '~ui/components/base/Button';
import PolyfrostFull from '~ui/components/logos/PolyfrostFull';
import { type JSX, type ParentProps } from 'solid-js';
import OnboardingLanguage from './OnboardingLanguage';
import OnboardingWelcome from './OnboardingWelcome';

const basePath = '/onboarding';
const OnboardingSteps = [
	['/', OnboardingWelcome],
	['/language', OnboardingLanguage],
] as const;

function Onboarding(props: ParentProps) {
	const navigate = useNavigate();
	const location = useLocation();

	const step = () => OnboardingSteps.findIndex(([path]) => location.pathname === basePath + path);

	const canGoBack = () => step() > 0;
	const canGoForward = () => step() < OnboardingSteps.length - 1;

	const next = () => {
		if (canGoForward())
			navigate(basePath + OnboardingSteps[step() + 1]![0]);
	};

	const previous = () => {
		if (canGoBack())
			navigate(basePath + OnboardingSteps[step() - 1]![0]);
	};

	return (
		<div class="h-full w-full flex flex-col p-8">
			<div class="">
				<PolyfrostFull />
			</div>

			<div class="flex-1 p-16">
				<AnimatedRoutes>
					{props.children}
				</AnimatedRoutes>
			</div>

			<div class="w-full flex flex-row items-end justify-end">
				<div class="w-1/3 flex flex-row items-stretch gap-x-8 [&>*]:w-full">
					<Button
						buttonStyle="secondary"
						children="Previous"
						disabled={!canGoBack()}
						iconLeft={<ChevronLeftIcon />}
						onClick={previous}
					/>

					<Button
						children={canGoForward() ? 'Next' : 'Finish'}
						iconRight={<ChevronRightIcon />}
						onClick={next}
					/>
				</div>
			</div>
		</div>
	);
}

Onboarding.Steps = OnboardingSteps;
Onboarding.Routes = OnboardingSteps.map(([path, component]) => (
	<Route component={component} path={path} />
));

export default Onboarding;

export interface OnboardingStepProps extends ParentProps {
	title: string;
	paragraph: string;
	illustration: JSX.Element;
}

export function OnboardingStep(props: OnboardingStepProps) {
	return (
		<div class="grid grid-cols-2 h-full w-full gap-x-16">
			<div class="flex flex-col items-center justify-center">
				{props.illustration}
			</div>

			<div class="flex flex-col gap-y-2">
				<div class="flex flex-col gap-y-2">
					<h1 class="text-2xl">{props.title}</h1>
					<p class="text-lg text-fg-secondary line-height-normal">{props.paragraph}</p>
				</div>

				<div class="flex flex-1 flex-col gap-y-2">
					{props.children}
				</div>
			</div>
		</div>
	);
}
