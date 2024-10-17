import type { Cluster, ImportType, License, Loader, ManagedPackage, PackageType, Providers, VersionType } from '@onelauncher/client/bindings';
import { DurationFormat } from '@formatjs/intl-durationformat';
import { open } from '@tauri-apps/plugin-shell';

export function setAsyncTimeout(ms: number): Promise<void>;
export function setAsyncTimeout(callback: () => void, ms: number): Promise<void>;

export function setAsyncTimeout(
	callback: (() => void) | number,
	ms?: number,
): Promise<void> {
	const _ms = typeof callback === 'number' ? callback : ms;

	return new Promise<void>((resolve) => {
		setTimeout(() => {
			if (typeof callback === 'function')
				callback();
			resolve();
		}, _ms);
	});
}

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

export function upperFirst<T extends string>(object: T): Capitalize<T> {
	const string = object.toString();
	return (string.charAt(0).toUpperCase() + string.slice(1)) as Capitalize<T>;
}

export function uint32ToBigInt([high, low]: [number, number]): bigint {
	return (BigInt(high >>> 0) << 32n) | BigInt(low >>> 0);
}

export function int32ToBigInt([high, low]: [number, number]): bigint {
	return (BigInt(high | 0) << 32n) | BigInt(low >>> 0);
}

export function abbreviateNumber(n: number | bigint, locale: string = 'en-US'): string {
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

export function formatAsDuration(
	seconds: number | bigint | Date,
	locale: string = 'en',
	style: 'long' | 'short' | 'narrow' | 'digital' = 'long',
): string {
	let n: number | undefined;

	if (seconds instanceof Date)
		n = seconds.getTime();
	else
		n = Number(seconds);

	const formatter = new DurationFormat(locale, { style });
	const duration = formatter.format({
		seconds: Math.floor(n % 60),
		minutes: Math.floor(n / 60) % 60,
		hours: Math.floor(n / (60 * 60)) % 24,
		days: Math.floor(n / (60 * 60 * 24)) % 7,
		weeks: Math.floor(n / (60 * 60 * 24 * 7)) % 4,
		months: Math.floor(n / (60 * 60 * 24 * 30)) % 12,
		years: Math.floor(n / (60 * 60 * 24 * 365)),
	});

	if (duration.length === 0)
		return 'never';

	return duration;
}

export function formatAsRelative(
	seconds: number | bigint | Date,
	locale: string = 'en',
	style: 'long' | 'short' | 'narrow' | 'digital' = 'long',
): string {
	let elapsed: number | undefined;

	if (seconds instanceof Date)
		elapsed = seconds.getTime();
	else
		elapsed = Number(seconds);

	elapsed -= Date.now();

	const units = {
		year: 31536000000,
		month: 2592000000,
		week: 604800000,
		day: 86400000,
		hour: 3600000,
		minute: 60000,
		second: 1000,
	};

	const formatter = new Intl.RelativeTimeFormat(locale, { style: style as Intl.RelativeTimeFormatStyle });
	for (const [unit, ms] of Object.entries(units))
		if (Math.abs(elapsed) > ms || unit === 'second')
			return formatter.format(Math.round(elapsed / ms), unit as Intl.RelativeTimeFormatUnit);

	return 'now';
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

export function getLicenseUrl(licenseId: string | License | null | undefined): string | undefined {
	if (licenseId === null || licenseId === undefined)
		return;

	let id: string | undefined;

	if (typeof licenseId !== 'string') {
		if (licenseId.url !== null && licenseId.url !== undefined) {
			open(licenseId.url);
			return;
		}

		id = licenseId.id || licenseId.name;
	}
	else {
		id = licenseId;
	}

	return `https://spdx.org/licenses/${id}.html`;
}

export function getPackageUrl(pkg: ManagedPackage): string {
	const mapping: Record<Providers, () => string> = {
		Modrinth: () => `https://modrinth.com/${pkg.package_type}/${pkg.id}`,
		Curseforge: () => {
			const packageTypeMapping: Record<PackageType, string> = {
				mod: 'mc-mods',
				shaderpack: 'shaders',
				resourcepack: 'texture-packs',
				datapack: 'data-packs',
				modpack: 'modpacks',
			};

			return `https://www.curseforge.com/minecraft/${packageTypeMapping[pkg.package_type]}/${pkg.main}`;
		},
	};

	return mapping[pkg.provider]();
}

export const LOADERS: Loader[] = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt'] as const;
export const PROVIDERS: Providers[] = ['Modrinth', 'Curseforge'] as const;
export const PACKAGE_TYPES: PackageType[] = ['mod', 'resourcepack', 'datapack', 'shaderpack'] as const;
export const LAUNCHER_IMPORT_TYPES: ImportType[] = ['PrismLauncher', 'Curseforge', 'Modrinth', 'ATLauncher', 'GDLauncher', 'FTBLauncher', 'MultiMC', 'TLauncher', 'Technic'] as const;
