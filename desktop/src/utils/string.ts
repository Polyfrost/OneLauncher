export function upperFirst(object: any): string {
	const str = object.toString();
	return str.charAt(0).toUpperCase() + str.slice(1);
}

export default {
	upperFirst,
};
