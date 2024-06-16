import { Route } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import BrowserMain from './BrowserMain';

function BrowserRoutes() {
	return (
		<>
			<Route path="/" component={BrowserMain} />
		</>
	);
}

function BrowserRoot(props: ParentProps) {
	return <>{props.children}</>;
}

BrowserRoot.Routes = BrowserRoutes;

export default BrowserRoot;
