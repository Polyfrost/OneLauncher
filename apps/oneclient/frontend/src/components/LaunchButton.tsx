import type { ButtonProps } from '@onelauncher/common/components';
import { NoAccountPopup, Overlay } from '@/components/overlay';
import { useIsRunning } from '@/hooks/useClusters';
import { useLaunchCluster } from '@/hooks/useLaunchCluster';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useState } from 'react';
import { tv } from 'tailwind-variants';

export type LaunchButtonProps = Omit<ButtonProps, 'children' | 'onClick'> & {
	clusterId: number | undefined | null;
};

const launchButtonVariants = tv({
	variants: {
		isRunning: {
			true: 'bg-success hover:bg-success-hover pressed:bg-success-pressed disabled:bg-success-disabled pending:bg-success-disabled',
		},
	},
});

export function LaunchButton({
	clusterId,
	isDisabled,
	className,
	...rest
}: LaunchButtonProps) {
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const launchCluster = useLaunchCluster(clusterId);
	const isRunning = useIsRunning(clusterId);
	const [open, setOpen] = useState<boolean>(false);

	const launch = () => {
		if (currentAccount === null) {
			setOpen(true);
		}
		else {
			launchCluster();
			setOpen(false);
		}
	};

	return (
		<Overlay.Trigger isOpen={open} onOpenChange={setOpen}>
			<Button
				className={launchButtonVariants({ isRunning, className })}
				isDisabled={isDisabled || isRunning}
				onPress={launch}
				{...rest}
			>
				{isRunning ? 'Running' : 'Launch'}
			</Button>

			<Overlay>
				<NoAccountPopup />
			</Overlay>
		</Overlay.Trigger>
	);
}
