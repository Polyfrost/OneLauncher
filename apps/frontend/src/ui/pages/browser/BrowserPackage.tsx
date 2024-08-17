import { type Params, useSearchParams } from '@solidjs/router';
import { Download01Icon } from '@untitled-theme/icons-solid';
import { onMount } from 'solid-js';
import type { Providers } from '~bindings';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import ModCard from '~ui/components/content/ModCard';
import useCommand from '~ui/hooks/useCommand';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

function BrowserPackage() {
	const [params] = useSearchParams<BrowserModParams>();
	const [pkg] = useCommand(bridge.commands.getPackage, params.id!);

	onMount(() => {

	});

	return (
		<>
			<div class="flex flex-row gap-x-12">
				<ModCard {...pkg()!} />
				<div class="flex flex-1 flex-col items-start justify-between rounded-lg bg-component-bg p-4 px-6">
					<div class="w-full flex flex-row items-center justify-between">
						<h1>{pkg()?.title}</h1>
						<Button
							buttonStyle="primary"
							iconLeft={<Download01Icon />}
							children="Install to..."
						/>
					</div>
					<p>TO DO</p>
				</div>
			</div>
		</>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `/browser/package?id=${params.id}&provider=${params.provider}`;
};

export default BrowserPackage;
