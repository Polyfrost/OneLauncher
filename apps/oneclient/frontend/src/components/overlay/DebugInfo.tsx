import type { DebugInfoArray } from '@/hooks/useDebugInfo';
import { Overlay } from '@/components';
import { copyDebugInfo, useDebugInfo } from '@/hooks/useDebugInfo';
import { Button } from '@onelauncher/common/components';

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
			<Button onPress={copyDebugInfo}>Copy Data</Button>
		</>
	);
}
