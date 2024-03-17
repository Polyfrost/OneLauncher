/* @refresh reload */
import { render } from 'solid-js/web';

import './globals.css';
import './fonts';
import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';
import BrowserPage from './ui/pages/Browser';
import UpdatesPage from './ui/pages/Updates';

render(() => (
	<Router root={App}>
		<Route component={HomePage} path="/" />
		<Route component={BrowserPage} path="/browser" />
		<Route component={UpdatesPage} path="/updates" />
	</Router>
), document.getElementById('root') as HTMLElement);
