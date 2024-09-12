import Illustration from '~assets/illustrations/onboarding/language.svg?component-solid';
import { OnboardingStep } from './Onboarding';

function OnboardingLanguage() {
	return (
		<OnboardingStep
			illustration={<Illustration />}
			paragraph="Choose your preferred language."
			title="Language"
		>
			<h1>lol</h1>
		</OnboardingStep>
	);
}

export default OnboardingLanguage;
