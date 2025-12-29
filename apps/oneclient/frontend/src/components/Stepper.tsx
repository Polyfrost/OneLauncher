import type { OnboardingStep } from '@/routes/onboarding/route';
import { twMerge } from 'tailwind-merge';

interface VerticalStepperProps {
	steps: Array<OnboardingStep>;
	currentStepIndex: number;
};

export function Stepper({ steps, currentStepIndex }: VerticalStepperProps) {
	return (
		<div className="max-w-md mx-auto">
			{steps.map((step, index) => (
				<div
					className={twMerge('after:transition-all after:duration-300 after:text-brand transition-all duration-300 relative flex items-center cursor-pointer py-2 pl-4 font-medium text-gray-400 text-base partial-leftline-0%  data-active:text-white data-active:text-lg data-active:partial-leftline-100% data-complete:partial-leftline-100%')}
					data-active={index === currentStepIndex || null}
					data-complete={index < currentStepIndex || null}
					key={step.path}
				>
					<span>{step.title}</span>
				</div>
			))}
		</div>
	);
}
