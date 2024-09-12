import { TRANSLATION_WEBSITE } from '@onelauncher/client';
import Illustration from '~assets/illustrations/onboarding/language.svg?component-solid';
import Link from '~ui/components/base/Link';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { For } from 'solid-js';
import { OnboardingStep } from './Onboarding';

function OnboardingLanguage() {
	const languages: [string, number][] = [
		['English', 100],
		['German', 21],
		['Spanish', 12],
		['Polish', 0],
		['Russian', 0],
		['German', 21],
		['Spanish', 12],
		['Polish', 0],
		['Russian', 0],
		['German', 21],
		['Spanish', 12],
		['Polish', 0],
		['Russian', 0],
		['German', 21],
		['Spanish', 12],
		['Polish', 0],
		['Russian', 0],
		['German', 21],
		['Spanish', 12],
		['Polish', 0],
		['Russian', 0],
	];

	const selected = () => languages[0];

	return (
		<OnboardingStep
			illustration={<Illustration />}
			paragraph="Choose your preferred language."
			title="Language"
		>
			<div class="h-full w-full flex flex-col gap-y-3">
				<div class="flex flex-col gap-y-2 rounded-lg bg-page-elevated">
					<OverlayScrollbarsComponent class="max-h-84 overflow-hidden">
						<div class="flex flex-col gap-y-1 p-2">
							<For each={languages.sort((a, b) => b[1] - a[1])}>
								{language => (
									<div
										class="flex flex-row items-center rounded-lg px-6 py-5"
										classList={{
											'bg-brand': language === selected(),
											'hover:bg-gray-05 active:bg-gray-10': language !== selected(),
										}}
									>
										<div class="flex-1 text-lg font-medium">{language[0]}</div>
										<div class="flex-1 text-right text-xs">
											{language[1]}
											%
										</div>
									</div>
								)}
							</For>
						</div>
					</OverlayScrollbarsComponent>
				</div>

				<div class="w-full flex flex-1 flex-row justify-end">
					<Link class="text-xs" href={TRANSLATION_WEBSITE}>Help translate OneLauncher</Link>
				</div>
			</div>
		</OnboardingStep>
	);
}

export default OnboardingLanguage;
