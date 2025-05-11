function OnboardingWelcome() {
	return (
		<div class="grid grid-cols-2 h-full w-full flex flex-col items-center justify-center gap-x-16">
			<h1 class="text-6xl">
				Welcome to
				{' '}
				<span class="underline underline-8 underline-brand">OneLauncher</span>
			</h1>

			<h3>A powerful yet easy to use launcher for Minecraft.</h3>
		</div>
	);
}

export default OnboardingWelcome;
