/* @refresh reload */
import { render } from 'solid-js/web';

import './imports';

import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import BrowserPage from './ui/pages/Browser';
import UpdatesPage from './ui/pages/Updates';
import ClusterRoot from '~ui/pages/cluster/ClusterRoot';
import SettingsRoot from '~ui/pages/settings/SettingsRoot';

export * as bridge from "./bindings";

render(() => (
    <Router root={App}>
        <Route path="/" component={HomePage} />
        <Route path="/browser" component={BrowserPage} />
        <Route path="/updates" component={UpdatesPage} />
        <Route path="/clusters" component={ClusterRoot}>
            <ClusterRoot.Routes />
        </Route>
        <Route path="/settings" component={SettingsRoot}>
            <SettingsRoot.Routes />
        </Route>
    </Router>
), document.getElementById('root') as HTMLElement);
