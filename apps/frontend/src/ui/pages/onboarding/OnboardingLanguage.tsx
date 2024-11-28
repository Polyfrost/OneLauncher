import { TRANSLATION_WEBSITE } from '@onelauncher/client';
import { useBeforeLeave } from '@solidjs/router';
import Illustration from '~assets/illustrations/onboarding/language.svg?component-solid';
import Link from '~ui/components/base/Link';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { createSignal, For } from 'solid-js';
import Onboarding, { OnboardingStep } from './Onboarding';

export const LanguagesList = {
	en: ['English', 100],
} as const;

export type Language = keyof typeof LanguagesList;

function OnboardingLanguage() {
	const ctx = Onboarding.useContext();

	const [selected, setSelected] = createSignal<Language>('en');

	const getSelectedLanguage = () => LanguagesList[selected()];

	useBeforeLeave(() => {
		ctx.setLanguage(selected());
	});

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
							<For each={Object.entries(LanguagesList).sort((a, b) => b[1][1] - a[1][1])}>
								{([code, language]) => (
									<div
										class="flex flex-row items-center rounded-lg px-6 py-5"
										classList={{
											'bg-brand': language === getSelectedLanguage(),
											'hover:bg-border/05 active:bg-border/10': language !== getSelectedLanguage(),
										}}
										onClick={() => setSelected(code as Language)}
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

				<div class="w-full flex flex-row justify-end">
					<Link class="text-xs" href={TRANSLATION_WEBSITE} skipPrompt>Help translate OneLauncher</Link>
				</div>
			</div>
		</OnboardingStep>
	);
}

export default OnboardingLanguage;
