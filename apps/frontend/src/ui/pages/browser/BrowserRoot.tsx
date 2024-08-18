import { Route } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import { Download01Icon, FileCode01Icon, SearchMdIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserPackage from './BrowserPackage';
import Button from '~ui/components/base/Button';
import TextField from '~ui/components/base/TextField';
import useBrowser from '~ui/hooks/useBrowser';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
			<Route path="/package" component={BrowserPackage} />
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
		<div class="sticky top-0 z-10 flex flex-row justify-between bg-page">
			<div class="flex flex-row gap-2">

				<TextField
					iconLeft={<SearchMdIcon />}
					placeholder="Search for content"
				/>

				<Button
					children={controller.cluster()?.meta.name || 'None'}
					iconLeft={<Settings01Icon />}
					onClick={controller.displayClusterSelector}
				/>

				{/* <Dropdown.Minimal
					onChange={sortable.setKey}
					icon={<FilterLinesIcon />}
				>
					<For each={Object.keys(sortable.sortables)}>
						{sortable => (
							<Dropdown.Row>{sortable}</Dropdown.Row>
						)}
					</For>
				</Dropdown.Minimal> */}

			</div>
			<div class="flex flex-row justify-end gap-2">
				<Button
					iconLeft={<Download01Icon />}
					children="From URL"
					buttonStyle="secondary"
				/>
				<Button
					iconLeft={<FileCode01Icon />}
					children="From File"
					buttonStyle="secondary"
				/>
			</div>
		</div>
	);
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
