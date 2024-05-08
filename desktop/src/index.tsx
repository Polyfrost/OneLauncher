/* @refresh reload */
import { render } from 'solid-js/web';

import './imports';

import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import BrowserPage from './ui/pages/Browser';
import UpdatesPage from './ui/pages/Updates';
import ClusterPage from './ui/pages/cluster';
import ClusterOverview from './ui/pages/cluster/ClusterOverview';
import ClusterMods from './ui/pages/cluster/ClusterMods';

render(() => (
	<Router root={App}>
		<Route path="/" component={HomePage} />
		<Route path="/browser" component={BrowserPage} />
		<Route path="/updates" component={UpdatesPage} />
		<Route path="/clusters" component={ClusterPage}>
			<Route path="/" component={ClusterOverview} />
			<Route path="/mods" component={ClusterMods} />
		</Route>
	</Router>
), document.getElementById('root') as HTMLElement);
