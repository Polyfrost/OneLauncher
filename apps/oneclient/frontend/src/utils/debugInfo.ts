import type { DebugInfoParsedLine } from '@/bindings.gen';
import type { ShortcutEvent } from '@tauri-apps/plugin-global-shortcut';
import { bindings } from '@/main';
import { toast } from '@/utils/toast';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { register } from '@tauri-apps/plugin-global-shortcut';
import { useEffect, useState } from 'react';

export type DebugInfoArray = Array<DebugInfoParsedLine>;
export function useDebugInfo(): DebugInfoArray {
	const [debugInfo, setDebugInfo] = useState<DebugInfoArray>([]);

	useEffect(() => {
		const fetchDevInfo = async () => {
			const info = await bindings.debug.getFullDebugInfoParsed();
			setDebugInfo(info);
		};

		void fetchDevInfo();
	}, []);

	return debugInfo;
}

export function copyDebugInfo(debugInfo: DebugInfoArray) {
	const timestamp = Math.floor(new Date().getTime() / 1000);
	const lines = [
		'## OneClient Debug Information',
		`**Data exported at:** <t:${timestamp}> (\`${timestamp}\`)`,
		...debugInfo.map((lineData) => {
			if (lineData.title === 'Build Timestamp')
				return `**${lineData.title}:** <t:${lineData.value}> (\`${lineData.value}\`)`;
			return `**${lineData.title}:** \`${lineData.value}\``;
		}),
	];
	writeText(lines.join('\n'));
	toast({
		type: 'info',
		title: 'Debug Info',
		message: 'Debug info has been copied to clipboard',
	});
}

export function useDebugKeybind() {
	const handleKeybind = async (event: ShortcutEvent) => {
		if (event.state !== 'Pressed')
			return;
		const info = await bindings.debug.getFullDebugInfoParsed();
		copyDebugInfo(info);
	};

	useEffect(() => {
		void (async () => {
			await register('CommandOrControl+Shift+D', handleKeybind);
			await register('Alt+F12', handleKeybind);
		})();
	}, []);
}
