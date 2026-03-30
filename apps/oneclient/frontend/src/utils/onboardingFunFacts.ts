import { ONBOARDING_TIPS_BACKUP } from '@/constants/onboardingTipsBackup';

const FUN_FACTS_URL = 'https://raw.githubusercontent.com/Polyfrost/DataStorage/main/oneclient/funfacts.txt';

let cachedOnboardingTips: Array<string> | null = null;
let onboardingTipsPromise: Promise<Array<string>> | null = null;

function normalizeLineBreakMarkers(value: string): string {
	return value
		.replaceAll('/n', '\n')
		.replaceAll('\\n', '\n');
}

function parseFunFacts(text: string): Array<string> {
	return text
		.split(/\r?\n/)
		.map(line => line.trim())
		.filter(Boolean)
		.map(normalizeLineBreakMarkers);
}

async function fetchOnboardingTips(): Promise<Array<string>> {
	const response = await fetch(FUN_FACTS_URL, { cache: 'no-store' });
	if (!response.ok)
		throw new Error(`Failed to fetch onboarding fun facts: ${response.status}`);

	const tips = parseFunFacts(await response.text());
	if (tips.length === 0)
		throw new Error('Fetched onboarding fun facts were empty.');

	return tips;
}

export function getCachedOnboardingTips(): Array<string> {
	return (cachedOnboardingTips ?? ONBOARDING_TIPS_BACKUP).map(normalizeLineBreakMarkers);
}

export async function loadOnboardingTips(): Promise<Array<string>> {
	if (cachedOnboardingTips)
		return cachedOnboardingTips;

	if (!onboardingTipsPromise)
		onboardingTipsPromise = fetchOnboardingTips()
			.then((tips) => {
				cachedOnboardingTips = tips;
				return tips;
			})
			.finally(() => {
				onboardingTipsPromise = null;
			});

	return onboardingTipsPromise;
}

export async function preloadOnboardingTips(): Promise<void> {
	try {
		await loadOnboardingTips();
	}
	catch (error) {
		console.warn('[onboarding] Failed to preload fun facts, using backup facts.', error);
	}
}
