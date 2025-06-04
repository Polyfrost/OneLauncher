import Illustration from '@/assets/illustrations/onboarding/language.svg';
import { createFileRoute } from '@tanstack/react-router';
import { OnboardingStep } from './route';

export const Route = createFileRoute('/onboarding/language')({
	component: RouteComponent,
});

const languages = [
	{
		lang: 'English',
		percentage: 100,
	},
];

function RouteComponent() {
	return (
		<>
			<OnboardingStep
				// importing and using it as a react component was not working
				illustration={<img src={Illustration} />}
				paragraph="Choose your preferred language."
				title="Language"
			>
				<div className="h-full w-full flex flex-col gap-y-3">
					<div className="flex flex-col gap-y-2 rounded-lg bg-page-elevated">
						<div className="max-h-84 overflow-hidden">
							<div className="flex flex-col gap-y-1 p-2">
								<aside>
									{languages.map(x => (
										<div
											className="flex flex-row items-center rounded-lg px-6 py-5"
											key={x.lang}
										>
											<div className="flex-1 text-lg font-medium">{x.lang}</div>
											<div className="flex-1 text-right text-xs">
												{x.percentage}
												%
											</div>
										</div>
									))}
								</aside>
							</div>
						</div>
					</div>

					<div className="w-full flex flex-row justify-end">
						<p className="text-xs">Help translate OneLauncher</p>
					</div>
				</div>
			</OnboardingStep>
		</>
	);
}
