export const LAUNCHER_IMPORT_TYPES: Array<string> = [
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

export function pluralize(n: number, word: string, locale: string = 'en'): string {
	const pluralRules = new Intl.PluralRules(locale);
	const pluralForm = pluralRules.select(n);
	return pluralForm === 'one' ? word : `${word}s`;
}
