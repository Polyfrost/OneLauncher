import type { DebugInfoParsedLine } from '@/bindings.gen';
import { Overlay } from '@/components';
import { bindings } from '@/main';
import { toast } from '@/utils/toast';
import { Button } from '@onelauncher/common/components';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
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

export function DebugInfo() {
	const debugInfo = useDebugInfo();
	return (
		<Overlay.Dialog>
			<Overlay.Title>Debug Info</Overlay.Title>
			<RawDebugInfo debugInfo={debugInfo} />
		</Overlay.Dialog>
	);
}

export function RawDebugInfo({ debugInfo }: { debugInfo: DebugInfoArray }) {
	const copy = () => copyDebugInfo(debugInfo);
	return (
		<>
			<div>
				{debugInfo.map((lineData) => {
					let line = '';
					if (lineData.title === 'Build Timestamp')
						line = `${lineData.title}: ${new Date(Number(lineData.value) * 1000).toString()}`;
					else line = `${lineData.title}: ${lineData.value}`;

					return <p key={lineData.title}>{line}</p>;
				})}
			</div>
			<Button onPress={copy}>Copy Data</Button>
		</>
	);
}
