import { Route } from '@solidjs/router';
import { For, type ParentProps } from 'solid-js';
import { SearchMdIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserPackage from './BrowserPackage';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import useBrowser from '~ui/hooks/useBrowser';
import { LOADERS } from '~utils';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
			<Route path="/package" component={BrowserPackage} children={<BrowserPackage.Routes />} />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return (
		<>{props.children}</>
	);
}

export function BrowserToolbar() {
	const controller = useBrowser();

	return (
		<div class="flex flex-row justify-between bg-page">
			<div class="flex flex-row gap-2">

				<Button
					buttonStyle="secondary"
					children={controller.cluster()?.meta.name || 'None'}
					iconLeft={<Settings01Icon />}
					onClick={controller.displayClusterSelector}
				/>

			</div>
			<div class="flex flex-row justify-end gap-2">
				<TextField
					iconLeft={<SearchMdIcon />}
					placeholder="Search for content"
				/>
			</div>
		</div>
	);
}

export function BrowserCategories() {
	interface Category {
		name: string;
		sub: [string, string][];
	};

	const categories: Category[] = [
		{
			name: 'Providers',
			sub: LOADERS.map(loader => [loader, `/${loader.toLowerCase()}`]),
		},
		{
			name: 'Providers',
			sub: Array(15).fill('Adventure').map((_, i) => [`Adventure ${i}`, '/']),
		},
		{
			name: 'Providers',
			sub: LOADERS.map(loader => [loader, `/${loader.toLowerCase()}`]),
		},
		{
			name: 'Providers',
			sub: Array(15).fill('Adventure').map((_, i) => [`Adventure ${i}`, '/']),
		},
	];

	return (
		<div class="top-0 h-fit min-w-50 flex flex-col gap-y-6">
			<For each={categories}>
				{category => (
					<div class="flex flex-col gap-y-2">
						<h3 class="my-1 text-sm text-fg-secondary font-medium uppercase">{category.name}</h3>
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
	);
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
