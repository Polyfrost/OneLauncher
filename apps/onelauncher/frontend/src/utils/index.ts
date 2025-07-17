export const LAUNCHER_IMPORT_TYPES = [
	'PrismLauncher',
	'Curseforge',
	// 'Modrinth',
	'ATLauncher',
	'GDLauncher',
	// 'FTBLauncher',
	'MultiMC',
	// 'TLauncher',
	// 'Technic'
] as const;

export const LOADERS = [
	'vanilla',
	'forge',
	'neoforge',
	'quilt',
	'fabric',
	'legacyfabric',
] as const;
export const PROVIDERS = ['Modrinth', 'Curseforge', 'SkyClient'] as const;
export const PACKAGE_TYPES = ['mod', 'resourcepack', 'datapack', 'shaderpack'] as const;

export function pluralize(n: number, word: string, locale: string = 'en'): string {
	const pluralRules = new Intl.PluralRules(locale);
	const pluralForm = pluralRules.select(n);
	return pluralForm === 'one' ? word : `${word}s`;
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

export function randomString(len: number = 6, charSet: string = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789') {
	let randomString = '';
	for (let i = 0; i < len; i++) {
		const randomPos = Math.floor(Math.random() * charSet.length);
		randomString += charSet.substring(randomPos, randomPos + 1);
	}
	return randomString;
}

export function abbreviateNumber(n: number | bigint, locale: string = 'en-US'): string {
	return new Intl.NumberFormat(locale, {
		notation: 'compact',
		compactDisplay: 'short',
		maximumFractionDigits: 1,
	}).format(n);
};

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

function convertSeconds(secondsInput: number): string {
	let remaining = secondsInput;

	const hours = Math.floor(remaining / 3600);
	remaining %= 3600;

	const minutes = Math.floor(remaining / 60);
	remaining %= 60;

	const seconds = remaining;

	const parts: Array<string> = [];

	if (hours)
		parts.push(`${hours} hours`);
	if (minutes)
		parts.push(`${minutes} minutes`);
	if (seconds)
		parts.push(`${seconds} seconds`);

	return parts.join(' ');
}

export function formatAsDuration(seconds: number | bigint | Date): string {
	if (seconds instanceof Date)
		seconds = seconds.getTime() / 1000;
	else
		seconds = Number(seconds);

	return convertSeconds(seconds);
}
