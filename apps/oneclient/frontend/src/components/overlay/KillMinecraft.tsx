import { Overlay } from '@/components';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';

export function KillMinecraft({ setOpen }: { setOpen: React.Dispatch<React.SetStateAction<boolean>> }) {
	const { data: foundAllProcess } = useCommandSuspense(['getRunningProcesses'], () => bindings.core.getRunningProcesses());
	const kill = () => {
		foundAllProcess.forEach(process => bindings.core.killProcess(process.pid));
		setOpen(false);
	};

	return (
		<Overlay.Dialog>
			<Overlay.Title>Minecraft is running</Overlay.Title>
			<p className="max-w-sm text-fg-secondary">Do you want to kill minecraft?</p>
			<Overlay.Buttons buttons={[{ color: 'danger', key: 'Yes', children: 'Yes', size: 'normal', onClick: kill }, { color: 'secondary', key: 'No', children: 'No', size: 'normal', slot: 'close' }]} />
		</Overlay.Dialog>
	);
}
