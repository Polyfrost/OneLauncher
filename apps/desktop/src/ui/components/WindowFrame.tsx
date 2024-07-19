import { Window } from '@tauri-apps/api/window';
import { ChevronLeftIcon, Maximize02Icon, MinusIcon, XCloseIcon } from '@untitled-theme/icons-solid';
import type { JSX } from 'solid-js';
import { createSignal, onMount } from 'solid-js';
import Modal from './overlay/Modal';
import Button from './base/Button';
import useSettingsContext from '~ui/hooks/useSettings';

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
	const settings = useSettingsContext();

	const maximize = () => Window.getCurrent().toggleMaximize();
	const minimize = () => Window.getCurrent().minimize();
	const quit = () => Window.getCurrent().close();
	const kill = () => Window.getCurrent().destroy();

	onMount(() => {
		Window.getCurrent().onCloseRequested((event) => {
			if (settings.hide_close_prompt !== true) {
				event.preventDefault();
				setModalVisible(true);
			}
		});
	});

	return (
		<div data-tauri-drag-region class="z-[99999] flex flex-row justify-between items-center w-screen h-8 bg-secondary gap-0.5 pr-0.5">
			<div class="flex flex-row items-center">
				<TitlebarButton icon={ChevronLeftIcon} onClick={() => history.back()} />
			</div>

			<div class="flex flex-row items-center justify-end">
				<TitlebarButton icon={MinusIcon} onClick={() => minimize()} />
				<TitlebarButton icon={Maximize02Icon} onClick={() => maximize()} />
				<TitlebarButton icon={XCloseIcon} onClick={() => quit()} danger />
			</div>

			<Modal.Simple
				title="Close OneLauncher?"
				visible={isModalVisible}
				setVisible={setModalVisible}
				buttons={[
					<Button buttonStyle="secondary" onClick={() => setModalVisible(false)}>No</Button>,
					<Button buttonStyle="danger" onClick={() => kill()}>Yes</Button>,
				]}
			/>
		</div>
	);
}

export default WindowFrame;
