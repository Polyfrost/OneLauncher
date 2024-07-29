/* @refresh reload */
import { render } from 'solid-js/web';

import './imports';

import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import UpdatesPage from './ui/pages/Updates';
import ClusterRoot from '~ui/pages/cluster/ClusterRoot';
import SettingsRoot from '~ui/pages/settings/SettingsRoot';
import BrowserRoot from '~ui/pages/browser/BrowserRoot';

render(() => (
	<Router root={App}>
		<Route path="/" component={HomePage} />
		<Route path="/updates" component={UpdatesPage} />
		<Route path="/clusters" component={ClusterRoot} children={<ClusterRoot.Routes />} />
		<Route path="/settings" component={SettingsRoot} children={<SettingsRoot.Routes />} />
		<Route path="/browser" component={BrowserRoot} children={<BrowserRoot.Routes />} />
	</Router>
), document.getElementById('root') as HTMLElement);
