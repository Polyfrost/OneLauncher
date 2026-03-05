// Credit - https://github.com/DuckySoLucky/hypixel-discord-chat-bridge/blob/d3ea84a26ebf094c8191d50b4954549e2dd4dc7f/src/contracts/helperFunctions.js#L216-L225
export function ReplaceVariables(template: string, variables: Record<string, any>) {
	return template.replace(/\{(\w+)\}/g, (match: any, name: string | number) => variables[name] ?? match);
}

// Credit - https://github.com/DuckySoLucky/hypixel-discord-chat-bridge/blob/52887f12fce3bc9ebe91befaf394f289abd234a1/src/contracts/helperFunctions.js#L260-L278
export function TitleCase(str: string): string {
	if (!str)
		return '';

	if (typeof str !== 'string')
		return '';

	return str
		.toLowerCase()
		.replaceAll('_', ' ')
		.replaceAll('-', ' ')
		.split(' ')
		.map(word => word.charAt(0).toUpperCase() + word.slice(1))
		.join(' ');
}
