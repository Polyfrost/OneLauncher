import type { ImportType } from '@onelauncher/client/bindings';
import Illustration from '~assets/illustrations/onboarding/import_from_others.svg?component-solid';
import LauncherIcon from '~ui/components/content/LauncherIcon';
import { LAUNCHER_IMPORT_TYPES } from '~utils';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For } from 'solid-js';
import Onboarding, { OnboardingStep } from './Onboarding';
import { createModal } from '~ui/components/overlay/Modal';

function OnboardingImport() {
	const ctx = Onboarding.useContext();

	const modal = createModal((props) => {

	});

	function displayImport(type: ImportType) {

	}


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
								<button class="flex flex-col items-center justify-center gap-y-4 rounded-md p-4 active:bg-gray-10 hover:bg-gray-05" onClick={() => displayImport(type)}>
									<LauncherIcon class="h-16 max-w-22 min-w-16" launcher={type} />
									<span class="text-lg font-medium">{type}</span>
								</button>
							)}
						</For>
					</div>
				</OverlayScrollbarsComponent>
			</div>
		</OnboardingStep>
	);
}

export default OnboardingImport;

function InstancesPickerModal() {
	
}
