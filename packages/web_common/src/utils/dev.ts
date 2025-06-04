/* eslint-disable node/prefer-global/process -- we dont need imports this way */
const env = (import.meta as unknown as { env: { DEV: boolean; MODE: string } }).env;

export const isDev = env.DEV
	|| env.MODE === 'development'
	|| (typeof window !== 'undefined' && (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1'))
	|| (typeof globalThis.process !== 'undefined' && globalThis.process.env.NODE_ENV === 'development');

export function registerDevExperience() {
	// Enable CTRL+R to reload the page in dev mode
	window.addEventListener('keypress', (e) => {
		if (e.getModifierState('Control') && e.key === 'r') {
			e.preventDefault();
			window.location.reload(); // tauri disables this
		}
	});
}
