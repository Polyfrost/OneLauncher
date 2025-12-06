import { Overlay } from '@/components';
import { bindings } from '@/main';
import { Button } from '@onelauncher/common/components';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useEffect, useState } from 'react';

export type DebugInfoArray = Array<{ title: string; value: string }>;
export interface DebugInfoData {
	inDev: boolean;
	platform: string;
	arch: string;
	family: string;
	locale: string;
	type: string;
	version: string;
	commitHash: string;
	buildTimestamp: string;
}

export function useDebugInfo(): DebugInfoArray {
	const [devInfo, setDevInfo] = useState<DebugInfoData>({
		inDev: false,
		platform: 'UNKNOWN',
		arch: 'UNKNOWN',
		family: 'UNKNOWN',
		locale: 'UNKNOWN',
		type: 'UNKNOWN',
		version: 'UNKNOWN',
		commitHash: 'UNKNOWN',
		buildTimestamp: 'UNKNOWN',
	});

	useEffect(() => {
		const fetchDevInfo = async () => {
			const [
				inDev,
				platform,
				arch,
				family,
				locale,
				type,
				version,
				commitHash,
				buildTimestamp,
			] = await Promise.all([
				bindings.debug.isInDev(),
				bindings.debug.getPlatform(),
				bindings.debug.getArch(),
				bindings.debug.getFamily(),
				bindings.debug.getLocale(),
				bindings.debug.getType(),
				bindings.debug.getVersion(),
				bindings.debug.getCommitHash(),
				bindings.debug.getBuildTimestamp(),
			]);

			setDevInfo({
				inDev,
				platform,
				arch,
				family,
				locale: locale ?? 'UNKNOWN',
				type,
				version,
				commitHash,
				buildTimestamp,
			});
		};

		void fetchDevInfo();
	}, []);

	return [
		{ title: 'inDev', value: devInfo.inDev ? 'yes' : 'no' },
		{ title: 'Platform', value: devInfo.platform },
		{ title: 'Arch', value: devInfo.arch },
		{ title: 'Family', value: devInfo.family },
		{ title: 'Locale', value: devInfo.locale },
		{ title: 'Type', value: devInfo.type },
		{ title: 'Version', value: devInfo.version },
		{ title: 'Commit Hash', value: devInfo.commitHash },
		{ title: 'Build Timestamp', value: devInfo.buildTimestamp },
	];
}

export function copyDebugInfo(debugInfo: DebugInfoArray) {
	const timestamp = Math.floor(new Date().getTime() / 1000);
	const lines = [`**Data exported at:** <t:${timestamp}> (\`${timestamp}\`)`, ...debugInfo.map((lineData) => {
		if (lineData.title === 'Build Timestamp')
			return `**${lineData.title}:** <t:${lineData.value}> (\`${lineData.value}\`)`;
		return `**${lineData.title}:** \`${lineData.value}\``;
	})];
	writeText(lines.join('\n'));
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
						line = `${lineData.title}: ${new Date(Number(lineData.value) * 1000)}`;
					else line = `${lineData.title}: ${lineData.value}`;

					return <p key={lineData.title}>{line}</p>;
				})}
			</div>
			<Button onPress={copy}>Copy Data</Button>
		</>
	);
}
