import type { OnboardingStep } from '@/routes/onboarding/route';

interface VerticalStepperProps {
	steps: Array<OnboardingStep>;
	currentLinearIndex: number;
	linearSteps: Array<Omit<OnboardingStep, 'subSteps'>>;
};

export function Stepper({ steps, linearSteps, currentLinearIndex }: VerticalStepperProps) {
	// const matchRoute = useMatchRoute();

	// const currentLinearIndex = linearSteps.findIndex(step =>
	// 	matchRoute({ to: step.path }));

	const progressHeight = currentLinearIndex >= 0 ? `${Math.min((currentLinearIndex + 0.5) / (linearSteps.length - 1), 1) * 100}%` : '0%';

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
									<span
										className="
                    transition-all duration-300 font-medium
					text-gray-400
					text-base
					data-active:text-white
					data-active:text-lg
					data-complete:text-base
                  "
										data-active={index === currentLinearIndex || null}
										data-complete={index < currentLinearIndex || null}
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
