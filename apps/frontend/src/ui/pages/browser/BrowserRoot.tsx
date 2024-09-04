import { Route } from '@solidjs/router';
import { For, type JSX, type ParentProps } from 'solid-js';
import { SearchMdIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import type { PackageType } from '@onelauncher/client/bindings';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserPackage from './BrowserPackage';
import BrowserSearch from './BrowserSearch';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import useBrowser from '~ui/hooks/useBrowser';
import { browserCategories } from '~utils/browser';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
			<Route path="/package" component={BrowserPackage} children={<BrowserPackage.Routes />} />
			<Route path="/search" component={BrowserSearch} />
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
					value={browser.searchQuery().query || ''}
					placeholder="Search for content"
					onKeyPress={onKeyPress}
				/>
			</div>
		</div>
	);
}

export interface BrowserSidebarCategory {
	name: string;
	sub: [string, string][];
};

function BrowserCategories(props: { categories: BrowserSidebarCategory[] }) {
	return (
		<div class="top-0 grid grid-cols-[1fr_auto] h-fit min-w-50 gap-y-6">
			<div />
			<div class="flex flex-col gap-y-6">
				<For each={props.categories.concat(browserCategories.provider())}>
					{category => (
						<div class="flex flex-col gap-y-2">
							<h6 class="my-1">{category.name}</h6>
							<For each={category.sub}>
								{sub => (
									<a
										href={sub[1]}
										class="text-md text-fg-primary capitalize hover:text-fg-secondary"
										children={sub[0]}
									/>
								)}
							</For>
						</div>
					)}
				</For>
			</div>
		</div>
	);
}

function BrowserSidebar() {
	const controller = useBrowser();

	return (
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
				<h6 class="my-1">Search Filters</h6>
			</div>
		</div>
	);
}

export type BrowserContentProps = ParentProps & {
	categories?: BrowserSidebarCategory[];
};

const categories = (packageType: PackageType) => browserCategories.byPackageType(packageType);

export function BrowserContent(props: BrowserContentProps) {
	const browser = useBrowser();

	return (
		<div class="relative h-full flex flex-1 flex-col items-center gap-2">
			<div class="h-full w-full max-w-screen-xl flex flex-1 flex-col items-center gap-y-2">
				<div class="grid grid-cols-[220px_auto_220px] w-full gap-x-6">
					<div />
					<BrowserToolbar />
					<div />
				</div>

				<div class="grid grid-cols-[220px_auto_220px] w-full max-w-screen-xl gap-x-6 pb-8">
					<BrowserCategories categories={categories(browser.packageType()).concat(props.categories || [])} />

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
