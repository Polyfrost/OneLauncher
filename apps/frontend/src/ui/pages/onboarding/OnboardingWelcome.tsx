import { OnboardingStep } from './Onboarding';

function OnboardingWelcome() {
	return (
		<OnboardingStep
			illustration={<span>todo :(</span>}
			paragraph="OneLauncher is a powerful and easy-to-use launcher for your applications."
			title="Welcome to OneLauncher!"
		/>
	);
}

export default OnboardingWelcome;
