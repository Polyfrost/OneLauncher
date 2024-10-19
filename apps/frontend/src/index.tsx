/* @refresh reload */
import { Route, Router } from '@solidjs/router';

/* eslint-disable perfectionist/sort-imports -- Import order does matter Here */
import '~utils/patches';
import '@fontsource/poppins/400.css';

import '@fontsource/poppins/500.css';
import '@fontsource/poppins/600.css';
import 'virtual:uno.css';
import '@unocss/reset/tailwind.css';
import 'overlayscrollbars/overlayscrollbars.css';
import '~styles/globals.css';
import '~styles/highlightjs.css';

import App from '~ui/pages/App';
import BrowserRoot from '~ui/pages/browser/BrowserRoot';
import ClusterRoot from '~ui/pages/cluster/ClusterRoot';

import Onboarding from '~ui/pages/onboarding/Onboarding';
import SettingsRoot from '~ui/pages/settings/SettingsRoot';
import { render } from 'solid-js/web';
import RootLayout from './RootLayout';
import HomePage from './ui/pages/Home';
import UpdatesPage from './ui/pages/Updates';
import { getProgramInfo, initProgramInfo } from '@onelauncher/client';

initProgramInfo().finally(() => {
	document.body.setAttribute('data-platform', getProgramInfo().platform);

	render(() => (
		<Router root={RootLayout}>
			<Route component={App}>
				<Route component={HomePage} path="/" />
				<Route component={UpdatesPage} path="/updates" />
				<Route children={<ClusterRoot.Routes />} component={ClusterRoot} path="/clusters" />
				<Route children={<SettingsRoot.Routes />} component={SettingsRoot} path="/settings" />
				<Route children={<BrowserRoot.Routes />} component={BrowserRoot} path="/browser" />
			</Route>

			<Route
				children={Onboarding.Routes}
				component={Onboarding}
				path="/onboarding"
			/>
		</Router>
	), document.getElementById('root') as HTMLElement);
});
