import { bindings } from '@/main';
import { copyDebugInfo } from '@/utils/debugInfo';
import { useCommand } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';

export function MadeBy() {
	const { data: version } = useCommand(['getPackageVersion'], () => bindings.debug.getPackageVersion());
	const { data: isInDev } = useCommand(['isInDev'], () => bindings.debug.isInDev());

	return (
		<Button
			className="flex flex-col items-start p-4 text-xs text-fg-secondary"
			color="ghost"
			onClick={copyDebugInfo}
		>
			<p>OneClient by Polyfrost</p>
			<p>Version {version}</p>
			<p>{isInDev ? 'Development' : 'Release'} Build</p>
		</Button>
	);
}
