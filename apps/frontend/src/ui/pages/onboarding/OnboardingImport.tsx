import { useBeforeLeave } from '@solidjs/router';
import Illustration from '~assets/illustrations/onboarding/import_from_others.svg?component-solid';
import Image from '~assets/logos/vanilla.png';
import { LAUNCHER_IMPORT_TYPES } from '~utils';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For } from 'solid-js';
import Onboarding, { OnboardingStep } from './Onboarding';

function OnboardingImport() {
	const ctx = Onboarding.useContext();

	useBeforeLeave(() => {
		ctx.setImportTypes([]);
	});

	return (
		<OnboardingStep
			illustration={<Illustration />}
			paragraph="Import your profiles from other launchers."
			title="Import"
		>
			<div class="h-full w-full flex flex-col gap-y-3">
				<OverlayScrollbarsComponent>
					<div class="grid grid-cols-3">
						<For each={LAUNCHER_IMPORT_TYPES}>
							{type => (
								<div class="flex flex-col items-center justify-center gap-y-4 rounded-md p-4 active:bg-gray-10 hover:bg-gray-05">
									<img alt={type} class="h-16" src={Image} />
									<span class="text-lg font-medium">{type}</span>
								</div>
							)}
						</For>
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</OnboardingStep>
	);
}

export default OnboardingImport;
