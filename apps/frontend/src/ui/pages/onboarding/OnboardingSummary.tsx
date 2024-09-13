import Onboarding from './Onboarding';

function OnboardingSummary() {
	const ctx = Onboarding.useContext();

	return (
		<div class="grid grid-cols-2 h-full w-full flex flex-col items-center justify-center gap-x-16">
			<h1 class="text-6xl">
				Chosen setup tasks
			</h1>

			<h3>blah blah.</h3>

			<span>
				Task Stage =
				{ctx.tasksStage()}
			</span>
		</div>
	);
}

export default OnboardingSummary;
