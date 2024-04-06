import { Window } from '@tauri-apps/api/window';
import { MinusIcon, XCloseIcon } from '@untitled-theme/icons-solid';
import type { JSX } from 'solid-js';
import { createSignal, onMount } from 'solid-js';
import Modal from './overlay/Modal';
import Button from './base/Button';

interface TitlebarButtonProps {
	icon: (any: any) => JSX.Element;
	onClick: (event: MouseEvent) => void;
	danger?: boolean;
}

function TitlebarButton(props: TitlebarButtonProps) {
	return (
		<button class="flex items-center justify-center w-8 h-8 group" onClick={e => props.onClick(e)}>
			<div class="rounded-lg group-hover:bg-primary p-1">
				<props.icon class={`w-[18px] h-[18px] stroke-slate ${props.danger ? 'group-hover:stroke-danger' : 'group-hover:stroke-white'}`} />
			</div>
		</button>
	);
}

function WindowFrame() {
	const [isModalVisible, setModalVisible] = createSignal(false);

	const minimize = () => Window.getCurrent().minimize();
	const quit = () => Window.getCurrent().destroy();

	onMount(() => {
		Window.getCurrent().onCloseRequested((event) => {
			event.preventDefault();
			setModalVisible(true);
		});
	});

	return (
		<div data-tauri-drag-region class="flex flex-row justify-between items-center w-screen h-8 bg-secondary gap-0.5 pr-0.5">
			<div class="flex flex-row items-center">
				{/* Empty */}
			</div>

			<div class="flex flex-row items-center justify-end">
				<TitlebarButton icon={MinusIcon} onClick={() => minimize()} />
				<TitlebarButton icon={XCloseIcon} onClick={() => setModalVisible(true)} danger />
			</div>

			<Modal
				title="Close OneLauncher?"
				visible={isModalVisible}
				setVisible={setModalVisible}
				buttons={[
					<Button styleType="secondary" onClick={() => setModalVisible(false)}>No</Button>,
					<Button styleType="danger" onClick={() => quit()}>Yes</Button>,
				]}
			/>
		</div>
	);
}

export default WindowFrame;
