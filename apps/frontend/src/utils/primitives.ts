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

export function secondsToWords(seconds: number | bigint): string {
	const n = Number(seconds);

	if (n < 60)
		return `${n} seconds`;
	else if (n < (60 * 60))
		return `${Math.floor(n / 60)} minutes`;
	else if (n < (60 * 60 * 24))
		return `${Math.floor(n / (60 * 60))} hours`;
	else if (n < (60 * 60 * 24 * 7))
		return `${Math.floor(n / (60 * 60 * 24))} days`;
	else if (n < (60 * 60 * 24 * 30))
		return `${Math.floor(n / (60 * 60 * 24 * 7))} weeks`;
	else if (n < (60 * 60 * 24 * 365))
		return `${Math.floor(n / (60 * 60 * 24 * 30))} months`;
	else if (n < (60 * 60 * 24 * 365 * 10))
		return `${Math.floor(n / (60 * 60 * 24 * 365))} years`;

	return `${n} seconds`; // Hmmmmmm
}

export default {
	upperFirst,
	abbreviateNumber,
};
