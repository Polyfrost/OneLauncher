import type { MinecraftCredentials } from '@/bindings.gen';
import { AccountAvatar, SkinViewer } from '@/components';
import { usePlayerProfile } from '@/hooks/usePlayerProfile';
import { bindings } from '@/main';
import { useCommandMut, useCommandSuspense } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { createFileRoute } from '@tanstack/react-router';
import { OnboardingNavigation } from './route';

export const Route = createFileRoute('/onboarding/account')({
	component: RouteComponent,
});

function Viewer({
	credentials,
}: {
	credentials: MinecraftCredentials;
}) {
	const { data: profile } = usePlayerProfile(credentials.id);
	return (
		<SkinViewer
			capeUrl={profile?.cape_url}
			className="h-full w-full max-w-1/4"
			height={400}
			skinUrl={profile?.skin_url}
			width={250}
		/>
	);
}

function AccountPreview({
	profile,
}: {
	profile: MinecraftCredentials;
}) {
	return (
		<div className="flex flex-row gap-2">
			<div>
				<Viewer credentials={profile} />
			</div>
			<div className="flex flex-row justify-start gap-2">
				<AccountAvatar className="aspect-square h-12 rounded-sm " uuid={profile.id} />
				<div className="text-left flex flex-col">
					<p className="flex items-center gap-1 text-fg-primary font-semibold">{profile.username}</p>
					<p className="text-fg-secondary text-sm">{profile.id}</p>
				</div>
			</div>
		</div>
	);
}

function RouteComponent() {
	const queryClient = useQueryClient();
	const { data: currentAccount } = useCommandSuspense(['getDefaultUser'], () => bindings.core.getDefaultUser(true));
	const { data: profile, isPending, mutate: login } = useCommandMut(bindings.core.openMsaLogin, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
	});

	const onClick = () => {
		login();
	};

	return (
		<>
			<div className="flex flex-col h-full px-12 gap-4">
				<div>
					<h1 className="text-4xl font-semibold mb-2">Account</h1>
					<p className="text-slate-400 text-lg mb-2">Before you continue, we require you to own a copy of Minecraft: Java Edition.</p>
				</div>
				{currentAccount
					? (
						<>
							<AccountPreview profile={currentAccount} />
						</>
					)
					: (
						<>
							{profile
								? (
									<>
										<AccountPreview profile={profile} />
									</>
								)
								: (
									<Button
										color="primary"
										isPending={isPending}
										onClick={onClick}
										size="large"
									>
										Add Account
									</Button>
								)}
						</>
					)}
			</div>
			<OnboardingNavigation disableNext={currentAccount === null} />
		</>
	);
}
