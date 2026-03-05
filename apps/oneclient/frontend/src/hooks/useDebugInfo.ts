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

export async function copyDebugInfo() {
	const info = await bindings.debug.getFullDebugInfoParsedString();
	writeText(info);
	toast({
		type: 'info',
		title: 'Debug Info',
		message: 'Debug info has been copied to clipboard',
	});
}

export function useDebugKeybind() {
	const handleKeybind = (event: ShortcutEvent) => {
		if (event.state !== 'Pressed')
			return;
		copyDebugInfo();
	};

	useEffect(() => {
		void (async () => {
			await register('CommandOrControl+Shift+D', handleKeybind);
			await register('Alt+F12', handleKeybind);
		})();
	}, []);
}
