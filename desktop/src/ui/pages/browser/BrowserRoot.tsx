import { Route } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import BrowserMain from './BrowserMain';
import BrowserCategory from './BrowserCategory';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
			<Route path="/category" component={BrowserCategory} />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return <>{props.children}</>;
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
