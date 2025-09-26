import type { MinecraftCredentials } from '@/bindings.gen';
import { AccountAvatar } from '@/components/AccountAvatar';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { Overlay } from './Overlay';

export function AddAccountModal() {
	const queryClient = useQueryClient();

	const { data: profile, isPending, mutate: login } = useCommandMut(bindings.core.openMsaLogin, {
		onSuccess() {
			queryClient.invalidateQueries({
				queryKey: ['getUsers'],
			});
			queryClient.invalidateQueries({
				queryKey: ['getDefaultUser'],
			});
		},
	});

	const onClick = () => {
		login();
	};

	return (
		<Overlay.Dialog>
			<Overlay.Title>Add Account</Overlay.Title>

			<p className="max-w-sm text-fg-secondary">
				Lorem ipsum dolor sit amet consectetur, adipisicing elit. Fugiat aut unde, rem esse natus iusto impedit doloribus laborum laboriosam amet? Totam, commodi sed ducimus dicta praesentium sunt? A, soluta iusto.
			</p>

			{profile
				? (
						<>
							<AccountRow profile={profile} />
							<Button
								className="w-full"
								color="primary"
								size="large"
								slot="close"
							>
								Close
							</Button>
						</>
					)
				: (
						<Button
							className="w-full"
							color="primary"
							isPending={isPending}
							onClick={onClick}
							size="large"
						>
							Add Account
						</Button>
					)}
		</Overlay.Dialog>
	);
}

function AccountRow({
	profile,
}: {
	profile: MinecraftCredentials;
}) {
	return (
		<div className="flex flex-row items-center justify-start gap-2">
			<AccountAvatar className="aspect-square h-12 rounded-sm " uuid={profile.id} />

			<div className="text-left flex flex-col">
				<p className="flex items-center gap-1 text-fg-primary font-semibold">{profile.username}</p>
				<p className="text-fg-secondary text-sm">{profile.id}</p>
			</div>
		</div>
	);
}
