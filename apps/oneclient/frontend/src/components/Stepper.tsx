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
					className={twMerge('transition-all duration-300 relative flex items-center cursor-pointer py-2 pl-4 border-l-3 border-gray-600 font-medium text-gray-400 text-base data-active:border-brand data-active:text-white data-active:text-lg data-complete:border-brand', index === 0 ? 'pt-0' : '', index === steps.length - 1 ? 'pb-0' : '')}
					data-active={index === currentStepIndex || null}
					data-complete={index < currentStepIndex || null}
					key={index}
				>
					<span>{step.title}</span>
				</div>
			))}
		</div>
	);
}
