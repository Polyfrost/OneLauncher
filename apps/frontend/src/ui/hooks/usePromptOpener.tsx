import { open } from '@tauri-apps/plugin-shell';
import { createSignal } from 'solid-js';
import Button from '~ui/components/base/Button';
import Modal, { createModal } from '~ui/components/overlay/Modal';

function usePromptOpener() {
	const [url, setUrl] = createSignal<string | null>(null);

	const modal = createModal(props => (
		<Modal.Simple
			{...props}
			title="Third Party Link"
			children={(
				<span class="m-auto max-w-60">You are about to visit an external website. Are you sure you want to continue?</span>
			)}
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

	return (url: string | undefined, force: boolean = false) => {
		if (url === undefined)
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
