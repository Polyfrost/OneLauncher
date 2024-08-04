import { type Params, useSearchParams } from '@solidjs/router';
import { Download01Icon } from '@untitled-theme/icons-solid';
import { For, createSignal, onMount } from 'solid-js';
import type { Providers } from '~bindings';
import { bridge } from '~imports';
import Button from '~ui/components/base/Button';
import Dropdown from '~ui/components/base/Dropdown';
import ModCard from '~ui/components/content/ModCard';
import Modal from '~ui/components/overlay/Modal';
import useCommand, { tryResult } from '~ui/hooks/useCommand';

interface BrowserModParams extends Params {
	id: string;
	provider: Providers;
}

// TODO: Refactor the entire browser page and subpages
function BrowserPackage() {
	const [params] = useSearchParams<BrowserModParams>();
	const _isInvalid = !params.id || !params.provider;
	const [visible, setVisible] = createSignal(false);
	const [cluster, setCluster] = createSignal<string>();
	const [clusters, setClusters] = createSignal<[string, string][]>();
	const [pkg] = useCommand(bridge.commands.getPackage, params.id!);

	function installTo() {
		setVisible(true);
	}

	function download() {
		setVisible(false);
		tryResult(bridge.commands.downloadPackage, cluster()!, params.id!).then().catch(err => console.error(err));
	}

	function onChange(selected: number) {
		setCluster(clusters()![selected]![1]);
	}

	onMount(() => {
		tryResult(bridge.commands.getClusters).then((res) => {
			const list: [string, string][] = res.map(cluster => [cluster.meta.name, cluster.uuid]);
			setClusters(list);
			setCluster(clusters()![0]![1]);
		});
	});

	return (
		<>
			<div class="flex flex-row gap-x-12">
				<ModCard {...pkg()!} />
				<div class="flex flex-1 flex-col justify-between items-start bg-component-bg rounded-lg p-4 px-6">
					<div class="flex w-full flex-row justify-between items-center">
						<h1>{pkg()?.title}</h1>
						<Button
							buttonStyle="primary"
							iconLeft={<Download01Icon />}
							children="Install to..."
							onClick={installTo}
						/>
					</div>
					<p>TO DO</p>
				</div>
			</div>

			<Modal.Simple
				visible={visible}
				setVisible={setVisible}
				title="Install To..."
				buttons={[
					<Button
						buttonStyle="secondary"
						children="Cancel"
						onClick={() => setVisible(false)}
					/>,
					<Button
						buttonStyle="primary"
						children="Download"
						onClick={download}
					/>,
				]}
			>
				<Dropdown onChange={onChange}>
					<For each={clusters()}>
						{cluster => (
							<Dropdown.Row>{cluster[0]}</Dropdown.Row>
						)}
					</For>
				</Dropdown>
			</Modal.Simple>

		</>
	);
}

BrowserPackage.buildUrl = function (params: BrowserModParams): string {
	return `/browser/package?id=${params.id}&provider=${params.provider}`;
};

export default BrowserPackage;
