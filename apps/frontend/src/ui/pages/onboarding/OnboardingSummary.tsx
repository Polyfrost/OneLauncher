import AnimatedRoutes from '~ui/components/AnimatedRoutes';
import { For, Match, Switch } from 'solid-js';
import Onboarding, { OnboardingTaskStage } from './Onboarding';

function OnboardingSummary() {
	const ctx = Onboarding.useContext();

	return (
		<div class="grid grid-cols-2 h-full w-full flex flex-col items-start justify-center gap-x-16 gap-y-2">
			<h1 class="text-6xl">
				Prepare OneLauncher
			</h1>

			<h3>Are you sure you want to proceed with the following tasks?</h3>

			<AnimatedRoutes>
				<Switch>
					<Match when={ctx.tasksStage() === OnboardingTaskStage.NotStarted}>
						<div class="my-8 max-h-36 w-full flex flex-1 flex-col gap-y-2 rounded-lg bg-page-elevated p-4 font-mono">
							<For each={ctx.getTasks()}>
								{task => (
									<span class="whitespace-pre text-lg text-fg-primary line-height-normal">
										{task}
									</span>
								)}
							</For>
						</div>
					</Match>
					<Match when>
						<div class="my-8 max-h-36 w-full flex flex-1 flex-col gap-y-2">
							<div class="h-full w-full flex items-center justify-center">
								<h2>Preparing...</h2>
							</div>
						</div>
					</Match>
				</Switch>
			</AnimatedRoutes>
		</div>
	);
}

export default OnboardingSummary;
