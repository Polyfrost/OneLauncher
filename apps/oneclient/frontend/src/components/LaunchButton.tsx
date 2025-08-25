import type { ButtonProps } from '@onelauncher/common/components';
import { useIsRunning } from '@/hooks/useClusters';
import { useLaunchCluster } from '@/hooks/useLaunchCluster';
import { Button } from '@onelauncher/common/components';
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
	const launch = useLaunchCluster(clusterId);
	const isRunning = useIsRunning(clusterId);

	return (
		<Button
			className={launchButtonVariants({ isRunning, className })}
			isDisabled={isDisabled || isRunning}
			onClick={launch}
			{...rest}
		>
			{isRunning ? 'Running' : 'Launch'}
		</Button>
	);
}
