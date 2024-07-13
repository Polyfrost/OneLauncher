import { Route } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';
import BrowserMod from './BrowserMod';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
			<Route path="/mod" component={BrowserMod} />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return <>{props.children}</>;
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
