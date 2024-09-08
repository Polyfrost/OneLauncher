import { Route, useLocation } from '@solidjs/router';
import { SearchMdIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import useBrowser from '~ui/hooks/useBrowser';
import { PROVIDERS } from '~utils';
import { browserCategories } from '~utils/browser';
import { For, type JSX, type ParentProps } from 'solid-js';
import BrowserMain from './BrowserMain';
import BrowserPackage from './BrowserPackage';
import BrowserSearch from './BrowserSearch';

function BrowserRoutes() {
	return (
		<>
			<Route component={BrowserMain} path="/" />
			<Route children={<BrowserPackage.Routes />} component={BrowserPackage} path="/package" />
			<Route component={BrowserSearch} path="/search" />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return (
		<>{props.children}</>
	);
}

function BrowserToolbar() {
	const browser = useBrowser();

	const onKeyPress: JSX.EventHandlerUnion<HTMLInputElement, KeyboardEvent> = (e) => {
		if (e.key === 'Enter') {
			const query = e.currentTarget.value;

			browser.setSearchQuery(prev => ({
				...prev,
				query,
			}));

			browser.search();
		}
	};

	return (
		<div class="w-full flex flex-row justify-between bg-page">
			<div class="flex flex-row gap-2" />

			<div class="flex flex-row gap-2">
				<TextField
					iconLeft={<SearchMdIcon />}
					onKeyPress={onKeyPress}
					placeholder="Search for content"
					value={browser.searchQuery().query || ''}
				/>
			</div>
		</div>
	);
}

function BrowserCategories() {
	const browser = useBrowser();
	const location = useLocation();

	const getEnabledCategories = () => browser.searchQuery().categories || [];

	const isEnabled = (category: string) => {
		return getEnabledCategories().includes(category);
	};

	const toggleCategory = (category: string) => {
		const enabledCategories = getEnabledCategories();
		const index = enabledCategories.indexOf(category);
		if (index > -1)
			enabledCategories.splice(index, 1);
		else
			enabledCategories.push(category);

		browser.setSearchQuery(prev => ({
			...prev,
			categories: enabledCategories,
		}));

		if (!location.pathname.includes('/search'))
			browser.search();
	};

	return (
		<div class="top-0 grid grid-cols-[1fr_auto] h-fit min-w-50 gap-y-6">
			<div />
			<div class="flex flex-col gap-y-6">
				<div class="flex flex-col gap-y-2">
					<h6 class="my-1">Categories</h6>
					<For each={browserCategories.byPackageType(browser.packageType())}>
						{category => (
							<p
								class={`text-md capitalize text-fg-primary hover:text-fg-primary-hover ${isEnabled(category.id) ? 'text-opacity-100! hover:text-opacity-90!' : 'text-opacity-60! hover:text-opacity-70!'}`}
								onClick={() => toggleCategory(category.id)}
							>
								{category.display}
							</p>
						)}
					</For>
				</div>
			</div>
		</div>
	);
}

function BrowserSidebar() {
	const controller = useBrowser();

	return (
		<div class="flex flex-col gap-y-4">
			<div class="flex flex-col gap-y-4">
				<div class="flex flex-col gap-y-1">
					<h6 class="my-1">Active Cluster</h6>
					<Button
						buttonStyle="secondary"
						children={controller.cluster()?.meta.name || 'None'}
						iconLeft={<Settings01Icon />}
						onClick={controller.displayClusterSelector}
					/>
				</div>
				<div class="flex flex-col gap-y-1">
					<h6 class="my-1">Provider</h6>
					<Dropdown
						onChange={index => controller.setSearchQuery(prev => ({
							...prev,
							provider: PROVIDERS[index] || 'Modrinth',
						}))}
						selected={() => PROVIDERS.indexOf(controller.searchQuery().provider)}
					>
						<For each={PROVIDERS}>
							{provider => (
								<Dropdown.Row>
									{provider}
								</Dropdown.Row>
							)}
						</For>
					</Dropdown>
				</div>
			</div>
		</div>
	);
}

export function BrowserContent(props: ParentProps) {
	return (
		<div class="relative h-full flex flex-1 flex-col items-center gap-2">
			<div class="h-full w-full max-w-screen-xl flex flex-1 flex-col items-center gap-y-2">
				<div class="grid grid-cols-[220px_auto_220px] w-full gap-x-6">
					<div />
					<BrowserToolbar />
					<div />
				</div>

				<div class="grid grid-cols-[220px_auto_220px] w-full max-w-screen-xl gap-x-6 pb-8">
					<BrowserCategories />

					<div class="h-full flex flex-col gap-y-4">
						<div class="h-full flex-1">
							{props.children}
						</div>
					</div>

					<BrowserSidebar />
				</div>
			</div>
		</div>
	);
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
