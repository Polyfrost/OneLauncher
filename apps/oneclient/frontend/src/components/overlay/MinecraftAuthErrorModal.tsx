import type { MinecraftAuthErrorInfo } from './minecraft-auth-errors';
import { Overlay } from '@/components';
import { bindings } from '@/main';
import { useCommandMut } from '@onelauncher/common';
import { Button } from '@onelauncher/common/components';
import { useQueryClient } from '@tanstack/react-query';
import { AlertTriangleIcon, Copy01Icon, RefreshCw01Icon } from '@untitled-theme/icons-react';
import { useState } from 'react';
import { minecraftAuthErrors } from './minecraft-auth-errors';

/**
 * Extracts an Xbox error code from a LauncherError or error string.
 * Error format from backend: "Minecraft authentication error: <CODE> during MSA step <STEP>"
 */
export function extractXboxErrorCode(error: unknown): string | null {
	const errorStr = typeof error === 'string'
		? error
		: error instanceof Error
			? error.message
			: typeof error === 'object' && error !== null
				? JSON.stringify(error)
				: String(error);

	const match = errorStr.match(/Minecraft authentication error:\s*(\d+)/);
	return match ? match[1] : null;
}

/**
 * Checks whether an error is a Minecraft/Xbox authentication error.
 */
export function isMinecraftAuthError(error: unknown): boolean {
	return extractXboxErrorCode(error) !== null;
}

function MatchedErrorContent({
	matchedError,
}: {
	matchedError: MinecraftAuthErrorInfo;
}) {
	return (
		<div className="flex flex-col gap-3 w-full text-left">
			<div>
				<h3 className="text-fg-primary font-semibold text-sm mb-1">What happened</h3>
				<p className="text-fg-secondary text-sm">{matchedError.whatHappened}</p>
			</div>

			<div>
				<h3 className="text-fg-primary font-semibold text-sm mb-1">How to fix it</h3>
				<ol className="list-none flex flex-col gap-1.5">
					{matchedError.stepsToFix.map((step, index) => (
						<li className="flex items-start gap-2" key={step}>
							<span className="flex-shrink-0 w-5 h-5 rounded-full bg-brand-primary/20 text-brand-primary text-xs flex items-center justify-center font-medium mt-0.5">
								{index + 1}
							</span>
							<span
								className="text-fg-secondary text-sm [&_a]:text-brand-primary [&_a]:underline"
								dangerouslySetInnerHTML={{ __html: step }}
							/>
						</li>
					))}
				</ol>
			</div>
		</div>
	);
}

function UnknownErrorContent() {
	return (
		<div className="flex flex-col gap-3 w-full text-left">
			<div>
				<h3 className="text-fg-primary font-semibold text-sm mb-1">Unknown error</h3>
				<p className="text-fg-secondary text-sm">
					We don't recognize this error and can't recommend specific steps to resolve it.
				</p>
				<p className="text-fg-secondary text-sm mt-1">
					Try visiting
					{' '}
					<a
						className="text-brand-primary underline"
						href="https://www.minecraft.net/en-us/login"
						rel="noopener noreferrer"
						target="_blank"
					>
						Minecraft Login
					</a>
					{' '}
					and signing in, as it may prompt you with the necessary steps.
				</p>
			</div>
		</div>
	);
}

export function MinecraftAuthErrorModal({
	error,
}: {
	error: unknown;
}) {
	const queryClient = useQueryClient();
	const [copied, setCopied] = useState(false);

	const errorCode = extractXboxErrorCode(error);
	const matchedError = errorCode
		? minecraftAuthErrors.find(e => e.errorCode === errorCode)
		: null;

	const { isPending: isRetrying, mutate: retryLogin } = useCommandMut(bindings.core.openMsaLogin, {
		onSuccess(data) {
			console.warn('[auth] MinecraftAuthErrorModal: retry openMsaLogin succeeded', data ? `user=${data.username} id=${data.id}` : 'returned null');
			queryClient.invalidateQueries({ queryKey: ['getUsers'] });
			queryClient.invalidateQueries({ queryKey: ['getDefaultUser'] });
		},
		onError(err) {
			console.error('[auth] MinecraftAuthErrorModal: retry openMsaLogin failed', err);
		},
	});

	const debugInfo = typeof error === 'string'
		? error
		: error instanceof Error
			? error.message
			: JSON.stringify(error, null, 2);

	const handleCopy = async () => {
		try {
			await navigator.clipboard.writeText(debugInfo);
			setCopied(true);
			setTimeout(() => setCopied(false), 2000);
		}
		catch {
			console.error('[auth] Failed to copy to clipboard');
		}
	};

	return (
		<Overlay.Dialog isDismissable>
			<div className="flex items-center gap-2">
				<div className="flex items-center justify-center w-8 h-8 rounded-full bg-danger/20">
					<AlertTriangleIcon className="w-4 h-4 text-danger" />
				</div>
				<Overlay.Title>Sign-in Error</Overlay.Title>
			</div>

			<p className="text-fg-secondary text-sm text-center max-w-sm">
				Something went wrong while signing in with your Microsoft account.
				{errorCode && (
					<>
						{' '}
						<span className="text-fg-secondary/60 text-xs">
							(Error code:
							{' '}
							{errorCode}
							)
						</span>
					</>
				)}
			</p>

			{matchedError ? <MatchedErrorContent matchedError={matchedError} /> : <UnknownErrorContent />}

			<div className="flex flex-col w-full gap-2 mt-1">
				<Button
					color="primary"
					isPending={isRetrying}
					onPress={() => retryLogin()}
					size="large"
					slot="close"
				>
					<RefreshCw01Icon className="w-4 h-4" />
					Sign in again
				</Button>

				<Button
					color="secondary"
					onPress={handleCopy}
					size="large"
				>
					<Copy01Icon className="w-4 h-4" />
					{copied ? 'Copied!' : 'Copy error details'}
				</Button>
			</div>
		</Overlay.Dialog>
	);
}
