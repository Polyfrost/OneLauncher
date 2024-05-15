/* @refresh reload */
import { render } from 'solid-js/web';

import './imports';

import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import BrowserPage from './ui/pages/Browser';
import UpdatesPage from './ui/pages/Updates';
import ClusterOverview from './ui/pages/cluster/ClusterOverview';
import ClusterMods from './ui/pages/cluster/ClusterMods';
import ClusterRoot from '~ui/pages/cluster/ClusterRoot';
import ClusterLogs from '~ui/pages/cluster/ClusterLogs';
import ClusterScreenshots from '~ui/pages/cluster/ClusterScreenshots';

render(() => (
	<Router root={App}>
		<Route path="/" component={HomePage} />
		<Route path="/browser" component={BrowserPage} />
		<Route path="/updates" component={UpdatesPage} />
		<Route path="/clusters" component={ClusterRoot}>
			<Route path="/" component={ClusterOverview} />
			<Route path="/logs" component={ClusterLogs} />
			<Route path="/mods" component={ClusterMods} />
            <Route path="/screenshots" component={ClusterScreenshots} />
		</Route>
	</Router>
), document.getElementById('root') as HTMLElement);
