import type { ClusterModel } from '@/bindings.gen';
import type { ButtonProps } from '@onelauncher/common/components';
import { KillMinecraft, NoAccountPopup, Overlay } from '@/components';
import { useIsRunning } from '@/hooks/useClusters';
import { useLaunchCluster } from '@/hooks/useLaunchCluster';
import { bindings } from '@/main';
import { useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { tv } from 'tailwind-variants';

export type LaunchButtonProps = Omit<ButtonProps, 'children' | 'onClick'> & {
	cluster: ClusterModel;
};

const launchButtonVariants = tv({
	variants: {
		isRunning: {
			true: 'bg-success hover:bg-success-hover pressed:bg-success-pressed disabled:bg-success-disabled pending:bg-success-disabled',
		},
	},
});

function PromptForOnboarding({ launch, setSkipPackagesCheck, cluster }: { launch: () => void; setSkipPackagesCheck: React.Dispatch<React.SetStateAction<boolean>>; cluster: ClusterModel }) {
	const navigate = useNavigate();
	const onClickYes = () => {
		localStorage.setItem('selectedClusters', JSON.stringify([{ mc_version: cluster.mc_version, mc_loader: cluster.mc_loader }]));
		navigate({ to: `/onboarding/preferences/versionCategory` });
	};

	const onClickLaunch = () => {
		setSkipPackagesCheck(true);
		launch();
	};

	return (
		<Overlay.Dialog>
			<Overlay.Title>Not Setup</Overlay.Title>
			<div className="flex flex-col items-center">
				<p className="max-w-sm text-fg-secondary">It seems like you haven't setup this version</p>
				<p className="max-w-sm text-fg-secondary">Do you want to setup this version?</p>
			</div>
			<Overlay.Buttons
				buttons={[
					{ color: 'primary', key: 'Yes', children: 'Yes', size: 'normal', onClick: onClickYes },
					{
						color: 'secondary',
						key: 'anyways',
						children: 'Launch Anyways',
						size: 'normal',
						onClick: onClickLaunch,
					},
				]}
			/>
		</Overlay.Dialog>
	);
}

export function LaunchButton({
	cluster,
	isDisabled,
	className,
	...rest
}: LaunchButtonProps) {
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: installedPackages } = useCommandSuspense(['getLinkedPackages', cluster.id], () => bindings.core.getLinkedPackages(cluster.id));
	const launchCluster = useLaunchCluster(cluster.id);
	const isRunning = useIsRunning(cluster.id);

	const [reason, setReason] = useState<'account' | 'packages' | 'kill' | null>(null);
	const [skipPackagesCheck, setSkipPackagesCheck] = useState(false);

	const launch = () => {
		if (isRunning)
			return setReason('kill');
		if (!currentAccount)
			return setReason('account');
		if (installedPackages.length === 0 && !skipPackagesCheck)
			return setReason('packages');

		launchCluster();
		setSkipPackagesCheck(false);
		setReason(null);
	};

	return (
		<Overlay.Trigger isOpen={reason !== null} onOpenChange={open => !open && setReason(null)}>
			<Button
				className={launchButtonVariants({ isRunning, className })}
				isDisabled={isDisabled}
				onPress={launch}
				{...rest}
			>
				{isRunning ? 'Running' : 'Launch'}
			</Button>

			<Overlay>
				{reason === 'kill' && <KillMinecraft setOpen={() => setReason(null)} />}
				{reason === 'packages' && <PromptForOnboarding cluster={cluster} launch={launch} setSkipPackagesCheck={setSkipPackagesCheck} />}
				{reason === 'account' && <NoAccountPopup />}
			</Overlay>
		</Overlay.Trigger>
	);
}
