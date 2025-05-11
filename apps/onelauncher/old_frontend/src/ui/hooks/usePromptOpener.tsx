import { open } from '@tauri-apps/plugin-shell';
import Button from '~ui/components/base/Button';
import Modal, { createModal } from '~ui/components/overlay/Modal';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-solid';
import { createSignal } from 'solid-js';

function usePromptOpener() {
	const [url, setUrl] = createSignal<string | null>(null);

	const modal = createModal(props => (
		<Modal.Simple
			{...props}
			buttons={[
				<Button
					buttonStyle="secondary"
					children="Cancel"
					onClick={cancel}
				/>,
				<Button
					buttonStyle="danger"
					children="Proceed"
					onClick={proceed}
				/>,
			]}
			children={(
				<div class="flex flex-col items-center gap-y-4">
					<span class="max-w-58">You are about to visit an external website that may malicious.</span>
					<span>Do you wish to proceed?</span>
					<OverlayScrollbarsComponent class="max-w-100">
						<div class="h-8 flex flex-row">
							<code>{url()}</code>
						</div>
					</OverlayScrollbarsComponent>
				</div>
			)}
			title="Third Party Link"
		/>
	));

	function cancel() {
		modal.hide();
	}

	function proceed() {
		modal.hide();

		if (url() !== null)
			open(url()!);
	}

	return (url: string | undefined | null, force: boolean = false) => {
		if (url === undefined || url === null)
			return;

		if (force) {
			open(url);
			return;
		}

		setUrl(url);
		modal.show();
	};
}

export default usePromptOpener;
