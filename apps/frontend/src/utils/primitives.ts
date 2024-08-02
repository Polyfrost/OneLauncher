export function upperFirst(object: any): string {
	const str = object.toString();
	return str.charAt(0).toUpperCase() + str.slice(1);
}

export function abbreviateNumber(n: number) {
	if (n < 1e3)
		return `${n}`;
	else if (n >= 1e3 && n < 1e6)
		return `${+(n / 1e3).toFixed(1)}K`;
	else if (n >= 1e6 && n < 1e9)
		return `${+(n / 1e6).toFixed(1)}M`;
	else if (n >= 1e9 && n < 1e12)
		return `${+(n / 1e9).toFixed(1)}B`;
	else if (n >= 1e12)
		return `${+(n / 1e12).toFixed(1)}T`;
	return `${n}`;
};

export function getEnumMembers(obj: any): string[] {
	return Object.keys(obj).filter((item) => {
		return Number.isNaN(Number(item));
	});
}

// pro lynith move
// TODO: Probably should use https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl/PluralRules
export function pluralize(n: number, word: string): string {
	return n === 1 ? word : `${word}s`;
}

export function secondsToWords(seconds: number | bigint, plural: boolean = true): string {
	const n = Number(seconds);

	const func = (n: number) => {
		if (n < 60)
			return `${n} second`;
		else if (n < (60 * 60))
			return `${Math.floor(n / 60)} minute`;
		else if (n < (60 * 60 * 24))
			return `${Math.floor(n / (60 * 60))} hour`;
		else if (n < (60 * 60 * 24 * 7))
			return `${Math.floor(n / (60 * 60 * 24))} day`;
		else if (n < (60 * 60 * 24 * 30))
			return `${Math.floor(n / (60 * 60 * 24 * 7))} week`;
		else if (n < (60 * 60 * 24 * 365))
			return `${Math.floor(n / (60 * 60 * 24 * 30))} month`;
		else if (n < (60 * 60 * 24 * 365 * 10))
			return `${Math.floor(n / (60 * 60 * 24 * 365))} year`;

		return `${n} second`; // Hmmmmmm
	};

	const str = func(n);
	if (plural) {
		const n = str.split(' ')[0];
		const word = str.split(' ')[1];
		return `${n} ${pluralize(Number(n), word as string)}`;
	}

	return str;
}

export default {
	upperFirst,
	abbreviateNumber,
};
