import { DurationFormat } from '@formatjs/intl-durationformat';
import type { Cluster, Loader, VersionType } from '~bindings';

export * from './timer';
export * from './sorting';

export function supportsMods(loader?: Cluster | Loader): boolean {
	if (loader === undefined)
		return false;

	if (typeof loader !== 'string')
		loader = loader.meta.loader;

	return loader !== 'vanilla';
}

export function formatVersionRelease(release: VersionType): string {
	const mapping: { [key in VersionType]: string } = {
		old_alpha: 'Alpha',
		old_beta: 'Beta',
		release: 'Release',
		snapshot: 'Snapshot',
	};

	return mapping[release];
}

export function upperFirst(object: any): string {
	const str = object.toString();
	return str.charAt(0).toUpperCase() + str.slice(1);
}

export function abbreviateNumber(n: number, locale: string = 'en-US'): string {
	return new Intl.NumberFormat(locale, {
		notation: 'compact',
		compactDisplay: 'short',
		maximumFractionDigits: 1,
	}).format(n);
};

export function pluralize(n: number, word: string, locale: string = 'en'): string {
	const pluralRules = new Intl.PluralRules(locale);
	const pluralForm = pluralRules.select(n);
	return pluralForm === 'one' ? word : `${word}s`;
}

export function secondsToWords(
	seconds: number | bigint,
	locale: string = 'en',
	style: 'long' | 'short' | 'narrow' | 'digital' = 'long',
): string {
	const n = Number(seconds);
	const formatter = new DurationFormat(locale, { style });
	return formatter.format({
		seconds: n % 60,
		minutes: Math.floor(n / 60) % 60,
		hours: Math.floor(n / (60 * 60)) % 24,
		days: Math.floor(n / (60 * 60 * 24)) % 7,
		weeks: Math.floor(n / (60 * 60 * 24 * 7)) % 4,
		months: Math.floor(n / (60 * 60 * 24 * 30)) % 12,
		years: Math.floor(n / (60 * 60 * 24 * 365)),
	});
}

export function asEnvVariables(str: string): [string, string][] {
	return str
		.split(' ')
		.map((pair) => {
			const [key, value = ''] = pair.split('=', 1);
			return key ? [key, value] as [string, string] : null;
		})
		.filter(pair => pair !== null);
}

export const LOADERS: Loader[] = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt'] as const;
