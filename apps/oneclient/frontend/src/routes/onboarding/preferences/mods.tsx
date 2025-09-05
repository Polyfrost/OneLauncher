import { Tab, TabContent, TabList, TabPanel, Tabs } from '@onelauncher/common/components';
import { createFileRoute } from '@tanstack/react-router';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';
import { twMerge } from 'tailwind-merge';

export const Route = createFileRoute('/onboarding/preferences/mods')({
	component: RouteComponent,
});

function RouteComponent() {
	return (
		<div className="min-h-screen px-7">
			<div className="max-w-6xl mx-auto">
				<h1 className="text-4xl font-semibold mb-2">Choose Mods</h1>
				<p className="text-slate-400 text-lg mb-2">
					Something something in corporate style fashion about picking your preferred gamemodes and versions and
					optionally loader so that oneclient can pick something for them
				</p>

				<div className="relative">
					<Tabs defaultValue="skyblock">
						<TabList className="gap-6">
							<Tab value="skyblock">Skyblock</Tab>
							<Tab value="survival">Survival</Tab>
							<Tab value="minigames">Minigames</Tab>
							<Tab value="pvp">PVP</Tab>
							<Tab value="gui">GUI</Tab>
							<Tab value="util">Utility</Tab>
							<Tab value="misc">Misc</Tab>

							<div className="absolute right-4">
								<div className="bg-[#D0D7F3] rounded-xl text-brand text-sm px-2 py-1">
									31 Mods selected
								</div>
							</div>
						</TabList>

						<TabContent>
							<TabPanel value="skyblock">
								<OverlayScrollbarsComponent>
									<div className="h-96 grid grid-cols-3 gap-2">
										{[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15].map(x => (
											<div className="space-y-2" key={x}>
												{x % 2 === 0 ? <ModCard2 active={x % 2 === 0} /> : <ModCard active={x % 2 === 0} />}
											</div>
										))}
									</div>
								</OverlayScrollbarsComponent>
							</TabPanel>
						</TabContent>
					</Tabs>
				</div>
			</div>
		</div>
	);
}

interface ModCardProps {
	active: boolean;
}

function ModCard(props: ModCardProps) {
	const { active } = props;

	return (
		<div className={twMerge('p-2 rounded-lg mb-2 break-inside-avoid', active ? 'bg-brand/20 border border-brand' : 'bg-component-bg border border-gray-100/5')}>
			<div className="flex flex-row gap-2">
				<img className="rounded-xl size-16" src="https://cdn.modrinth.com/data/8pJYUDNi/790dfffda5974fccda843477cfe8ed19b1347ea3_96.webp" />
				<div className="flex flex-col">
					<p className="text-fg-primary text-xl">Chatting</p>
					<p className="text-fg-secondary">
						by
						{' '}
						{' '}
						<span className="font-semibold">Polyfrost</span>
					</p>
				</div>
			</div>
			<div className="pt-1 text-fg-secondary font-normal">
				<p>Chatting is a chat mod adding utilities such as extremely customizable chat tabs, chat shortcuts, chat screenshots, and message copying.</p>
			</div>
		</div>
	);
}

function ModCard2(props: ModCardProps) {
	const { active } = props;

	return (
		<div className={twMerge('p-2 rounded-lg mb-2 break-inside-avoid', active ? 'bg-brand/20 border border-brand' : 'bg-component-bg border border-gray-100/5')}>
			<div className="flex flex-row gap-2">
				<img className="rounded-xl size-16" src="https://cdn.modrinth.com/data/8pJYUDNi/790dfffda5974fccda843477cfe8ed19b1347ea3_96.webp" />
				<div className="flex flex-col">
					<p className="text-fg-primary text-xl">Chatting</p>
					<p className="text-fg-secondary">
						by
						{' '}
						{' '}
						<span className="font-semibold">Polyfrost</span>
					</p>
				</div>
			</div>
			<div className="pt-1 text-fg-secondary font-normal">
				<p>Chatting is a chat mod adding utilities such as extremely customizable chat tabs, chat shortcuts, chat screenshots, and message copying. Chatting is a chat mod adding utilities such as extremely customizable chat tabs, chat shortcuts, chat screenshots, and message copying Chatting is a chat mod adding utilities such as extremely customizable chat tabs, chat shortcuts, chat screenshots, and message copying</p>
			</div>
		</div>
	);
}
