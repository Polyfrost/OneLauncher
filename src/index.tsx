/* @refresh reload */
import { render } from 'solid-js/web';

import './assets/fonts/Poppins/index.css';
import './globals.css';
import { Route, Router } from '@solidjs/router';
import App from './ui/App';
import HomePage from './ui/pages/Home';

render(() => (
    <Router root={App}>
        <Route component={HomePage} path={'/'} />
    </Router>
), document.getElementById('root') as HTMLElement);
