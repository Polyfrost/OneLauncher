import type { MinecraftCredentials } from '@onelauncher/client/bindings';
import MicrosoftLogo from '~assets/logos/microsoft.svg?component-solid';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import PlayerHead from '~ui/components/game/PlayerHead';
import useAccountController from '~ui/components/overlay/account/AddAccountModal';
import { createSignal, onMount, Show } from 'solid-js';
import Onboarding from './Onboarding';

function OnboardingLogin() {
	const context = Onboarding.useContext();
	const accountController = useAccountController();

	const [errorMessage, setErrorMessage] = createSignal<string>('');
	const [profile, setProfile] = createSignal<MinecraftCredentials>();

	onMount(() => {
		context.setForwardButtonEnabled(false);
	});

	async function requestLogin() {
		const result = await bridge.commands.authLogin();

		if (result.status === 'error') {
			setErrorMessage(result.error);
			return;
		}

		if (!result.data) {
			setErrorMessage('No account was found. Please try again.');
			return;
		}

		setProfile(result.data);
		accountController.refetch();

		context.setForwardButtonEnabled(true);
	}

	return (
		<div class="grid grid-cols-2 h-full w-full flex flex-col items-start justify-center gap-x-16 gap-y-4">
			<h1 class="text-6xl -mb-2">Login</h1>

			<h3>Before you continue, we require you to own a copy of Minecraft: Java Edition.</h3>

			<Show when={profile()}>
				<div class="w-full flex flex-row items-center justify-start gap-x-3 border border-border/05 rounded-lg bg-component-bg p-3">
					<PlayerHead class="rounded-md" uuid={profile()?.id} />
					<div class="flex flex-col gap-y-2">
						<p class="text-lg text-fg-primary">{profile()?.username}</p>
						<p class="text-fg-secondary">{profile()?.id}</p>
					</div>
				</div>
			</Show>

			<Button
				buttonStyle="secondary"
				children="Login with Microsoft"
				iconLeft={<MicrosoftLogo />}
				onClick={requestLogin}
			/>

			<p class="text-danger">{errorMessage()}</p>

		</div>
	);
}

export default OnboardingLogin;
