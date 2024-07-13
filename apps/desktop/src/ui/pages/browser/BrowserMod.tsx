import { type Params, useSearchParams } from '@solidjs/router';
import { Match, Switch } from 'solid-js';

interface BrowserModParams extends Params {
	id: string;
	provider: string;
}

function BrowserMod() {
	const [params] = useSearchParams<BrowserModParams>();
	const isInvalid = !params.id || !params.provider;

	return (
		<Switch>
			<Match when={isInvalid === true}>
				{/* TODO */}
				<div>uh oh this doesnt exist</div>
			</Match>
			<Match when={isInvalid === false}>
				<div>
					{JSON.stringify(params)}
				</div>
			</Match>
		</Switch>
	);
}

BrowserMod.getUrl = function (params: BrowserModParams): string {
	return `/browser/mod?id=${params.id}&fprovider=${params.provider}`;
};

export default BrowserMod;
