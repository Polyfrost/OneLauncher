import type { OnboardingStep } from '@/routes/onboarding/route';
import { twMerge } from 'tailwind-merge';

interface VerticalStepperProps {
	steps: Array<OnboardingStep>;
	currentLinearIndex: number;
};

export function Stepper({ steps, currentLinearIndex }: VerticalStepperProps) {
	return (
		<div>
			<div className="max-w-md mx-auto">
				<div className="relative">
					<ul className="relative">
						{steps.map((step, index) => (
							<li
								className={twMerge('transition-all duration-300 relative flex items-center cursor-pointer py-2 pl-4 border-l-3 border-gray-600 font-medium text-gray-400 text-base data-active:border-brand data-active:text-white data-active:text-lg data-complete:border-brand', index === 0 ? 'pt-0' : '', index === steps.length - 1 ? 'pb-0' : '')}
								data-active={index === currentLinearIndex || null}
								data-complete={index < currentLinearIndex || null}
								key={index}
							>
								<span>{step.title}</span>
							</li>
						))}
					</ul>
				</div>
			</div>
		</div>
	);
}
