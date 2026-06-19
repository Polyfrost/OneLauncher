import { AccountAvatar, Overlay } from '@/components';
import { bindings } from '@/main';
import { useCommand, useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import {
	AlertTriangleIcon,
	RefreshCw01Icon,
} from '@untitled-theme/icons-react';
import { useMemo, useState } from 'react';

function getErrorMessage(error: unknown): string {
	if (typeof error === 'string')
		return error;

	if (error instanceof Error)
		return error.message;

	if (typeof error === 'object' && error !== null)
		return JSON.stringify(error);

	return String(error);
}

function isExpiredRefreshError(error: unknown): boolean {
	const message = getErrorMessage(error);
	return (
		message.includes('invalid_grant')
		|| message.includes('grant is expired')
		|| message.includes('RefreshOAuthToken')
	);
}

export function AuthRefreshPrompt() {
	const queryClient = useQueryClient();
	const [dismissed, setDismissed] = useState(false);

	const refresh = useCommand(
		['refreshAccounts'],
		bindings.core.refreshAccounts,
		{
			retry: false,
			refetchOnWindowFocus: false,
		},
	);
	const shouldPrompt = useMemo(
		() => !dismissed && refresh.isError && isExpiredRefreshError(refresh.error),
		[dismissed, refresh.error, refresh.isError],
	);

	const {
		data: profile,
		isPending,
		mutate: signInAgain,
	} = useCommandMut(bindings.core.openMsaLogin, {
		onSuccess() {
			setDismissed(true);
			queryClient.invalidateQueries({ queryKey: ['getUsers'] });
			queryClient.invalidateQueries({ queryKey: ['getDefaultUser'] });
			queryClient.invalidateQueries({ queryKey: ['refreshAccounts'] });
		},
	});

	if (!shouldPrompt)
		return null;

	return (
		<Overlay
			isDismissable
			isOpen={shouldPrompt}
			onOpenChange={open => setDismissed(!open)}
		>
			<Overlay.Dialog isDismissable>
				<div className="flex items-center gap-2">
					<div className="flex items-center justify-center w-8 h-8 rounded-full bg-warning/20">
						<AlertTriangleIcon className="w-4 h-4 text-code-warn" />
					</div>
					<Overlay.Title>Sign In Again</Overlay.Title>
				</div>

				<p className="text-fg-secondary text-sm text-center max-w-sm">
					Your Microsoft session expired. Sign in again to keep your account and
					launch access up to date.
				</p>

				{profile && (
					<div className="flex flex-row items-center justify-start gap-2">
						<AccountAvatar
							className="aspect-square h-12 rounded-sm"
							uuid={profile.id}
						/>
						<div className="text-left flex flex-col">
							<p className="flex items-center gap-1 text-fg-primary font-semibold">
								{profile.username}
							</p>
							<p className="text-fg-secondary text-sm">{profile.id}</p>
						</div>
					</div>
				)}

				<Button
					className="w-full"
					color="primary"
					isPending={isPending}
					onPress={() => signInAgain()}
					size="large"
				>
					<RefreshCw01Icon className="w-4 h-4" />
					Sign in again
				</Button>
			</Overlay.Dialog>
		</Overlay>
	);
}
