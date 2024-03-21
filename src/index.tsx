/* @refresh reload */
import { render } from 'solid-js/web';

import './global_assets';

import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import BrowserPage from './ui/pages/Browser';
import UpdatesPage from './ui/pages/Updates';
import InstancePage from './ui/pages/instances';
import InstanceOverview from './ui/pages/instances/InstanceOverview';
import InstanceMods from './ui/pages/instances/InstanceMods';

render(() => (
	<Router root={App}>
		<Route path="/" component={HomePage} />
		<Route path="/browser" component={BrowserPage} />
		<Route path="/updates" component={UpdatesPage} />
		<Route path="/instances" component={InstancePage}>
			<Route path="/" component={InstanceOverview} />
			<Route path="/mods" component={InstanceMods} />
		</Route>
	</Router>
), document.getElementById('root') as HTMLElement);
