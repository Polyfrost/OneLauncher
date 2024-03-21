import { Route } from '@solidjs/router';
import InstanceMods from './InstanceMods';

function InstanceOverview() {
	return (
		<div>
			<h1>Hello World</h1>
		</div>
	);
}

InstanceOverview.prototype.getRoutes = function () {
	return (
		<>
			<Route path="/" component={InstanceOverview} />
			<Route path="/mods" component={InstanceMods} />
		</>
	);
};

export default InstanceOverview;
