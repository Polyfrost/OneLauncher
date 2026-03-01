import type { DebugInfoArray } from '@/utils/debugInfo';
import { Overlay } from '@/components';
import { copyDebugInfo, useDebugInfo } from '@/utils/debugInfo';
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
