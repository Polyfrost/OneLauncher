import type { OnboardingStep } from '@/routes/onboarding/route';
import { useMatchRoute } from '@tanstack/react-router';

interface VerticalStepperProps {
	steps: Array<OnboardingStep>;
	linearSteps: Array<Omit<OnboardingStep, 'subSteps'>>;
};

export function Stepper({ steps, linearSteps }: VerticalStepperProps) {
	const matchRoute = useMatchRoute();

	const currentLinearIndex = linearSteps.findIndex(step =>
		matchRoute({ to: step.path }));

	const progressHeight = currentLinearIndex >= 0 ? `${(currentLinearIndex / (linearSteps.length - 1)) * 100}%` : '0%';

	// const nextStep = () => {
	// 	if (currentStep < steps.length - 1)
	// 		setCurrentStep(currentStep + 1);
	// };

	// const prevStep = () => {
	// 	if (currentStep > 0)
	// 		setCurrentStep(currentStep - 1);
	// };

	// const goToStep = (stepIndex: number) => {
	// 	setCurrentStep(stepIndex);
	// };

	return (
		<div>
			<div className="max-w-md mx-auto">
				<div className="relative">
					<div className="absolute left-4 top-0 w-0.5 h-full bg-gray-600"></div>

					<div
						className="absolute left-4 top-0 w-0.5 bg-brand transition-all duration-500 ease-in-out"
						style={{
							height: `${progressHeight}`,
						}}
					>
					</div>

					<ul className="relative space-y-4">
						{steps.map((step, index) => (
							<li
								className="relative flex items-center cursor-pointer group"
								key={index}
								// onClick={() => goToStep(index)}
							>
								<div className="ml-8">
									<span className={`
                    transition-all duration-300 font-medium
                    ${index === currentLinearIndex
								? 'text-white text-lg'
								: index < currentLinearIndex
									? ' text-base'
									: 'text-gray-400 text-base'
							}
                  `}
									>
										{step.title}
									</span>
								</div>
							</li>
						))}
					</ul>
				</div>
			</div>
		</div>
	);
}
