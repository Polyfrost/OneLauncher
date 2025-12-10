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
	osVersion: string;
	commitHash: string;
	buildTimestamp: string;
	buildVersion: string;
}

export function useDebugInfo(): DebugInfoArray {
	const [devInfo, setDevInfo] = useState<DebugInfoData>({
		inDev: false,
		platform: 'UNKNOWN',
		arch: 'UNKNOWN',
		family: 'UNKNOWN',
		locale: 'UNKNOWN',
		type: 'UNKNOWN',
		osVersion: 'UNKNOWN',
		commitHash: 'UNKNOWN',
		buildTimestamp: 'UNKNOWN',
		buildVersion: 'UNKNOWN',
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
				osVersion,
				commitHash,
				buildTimestamp,
				buildVersion,
			] = await Promise.all([
				bindings.debug.isInDev(),
				bindings.debug.getPlatform(),
				bindings.debug.getArch(),
				bindings.debug.getFamily(),
				bindings.debug.getLocale(),
				bindings.debug.getType(),
				bindings.debug.getOsVersion(),
				bindings.debug.getGitCommitHash(),
				bindings.debug.getBuildTimestamp(),
				bindings.debug.getPackageVersion(),
			]);

			setDevInfo({
				inDev,
				platform,
				arch,
				family,
				locale: locale ?? 'UNKNOWN',
				type,
				osVersion,
				commitHash,
				buildTimestamp: new Date(buildTimestamp).getTime().toString(),
				buildVersion,
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
		{ title: 'Os Version', value: devInfo.osVersion },
		{ title: 'Commit Hash', value: devInfo.commitHash },
		{ title: 'Build Timestamp', value: devInfo.buildTimestamp },
		{ title: 'Version', value: devInfo.buildVersion },
	];
}

export function copyDebugInfo(debugInfo: DebugInfoArray) {
	const timestamp = Math.floor(new Date().getTime() / 1000);
	const lines = [`**Data exported at:** <t:${timestamp}> (\`${timestamp}\`)`, ...debugInfo.map((lineData) => {
		if (lineData.title === 'Build Timestamp')
			return `**${lineData.title}:** <t:${Math.floor(Number(lineData.value) / 1000)}> (\`${Math.floor(Number(lineData.value) / 1000)}\`)`;
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
						line = `${lineData.title}: ${new Date(Number(lineData.value))}`;
					else line = `${lineData.title}: ${lineData.value}`;

					return <p key={lineData.title}>{line}</p>;
				})}
			</div>
			<Button onPress={copy}>Copy Data</Button>
		</>
	);
}
