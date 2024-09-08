import { Window } from '@tauri-apps/api/window';
import { ChevronLeftIcon, Maximize02Icon, MinusIcon, XCloseIcon } from '@untitled-theme/icons-solid';
import useSettings from '~ui/hooks/useSettings';
import { onMount } from 'solid-js';
import type { JSX } from 'solid-js';
import Button from './base/Button';
import Modal, { createModal } from './overlay/Modal';

interface TitlebarButtonProps {
	icon: (any: any) => JSX.Element;
	onClick: (event: MouseEvent) => void;
	danger?: boolean;
}

function TitlebarButton(props: TitlebarButtonProps) {
	return (
		<button class="group h-8 w-8 flex items-center justify-center" onClick={e => props.onClick(e)}>
			<div class="rounded-lg p-1 group-hover:bg-page">
				<props.icon class={`w-[18px] h-[18px] stroke-slate ${props.danger ? 'group-hover:stroke-danger' : 'group-hover:stroke-white'}`} />
			</div>
		</button>
	);
}

const maximize = () => Window.getCurrent().toggleMaximize();
const minimize = () => Window.getCurrent().minimize();
const quit = () => Window.getCurrent().close();
const kill = () => Window.getCurrent().destroy();

function WindowFrame() {
	const { settings } = useSettings();

	const modal = createModal(props => (
		<Modal.Simple
			{...props}
			buttons={[
				<Button buttonStyle="secondary" onClick={() => props.hide()}>No</Button>,
				<Button buttonStyle="danger" onClick={() => kill()}>Yes</Button>,
			]}
			title="Close OneLauncher?"
		/>
	));

	onMount(() => {
		Window.getCurrent().onCloseRequested((event) => {
			if (settings().hide_close_prompt !== true) {
				event.preventDefault();
				modal.show();
			}
		});
	});

	return (
		<div class="z-[99999] h-8 w-screen flex flex-row items-center justify-between gap-0.5 bg-page-elevated pr-0.5" data-tauri-drag-region>
			<div class="flex flex-row items-center">
				<TitlebarButton icon={ChevronLeftIcon} onClick={() => history.back()} />
			</div>

			<div class="flex flex-row items-center justify-end">
				<TitlebarButton icon={MinusIcon} onClick={() => minimize()} />
				<TitlebarButton icon={Maximize02Icon} onClick={() => maximize()} />
				<TitlebarButton danger icon={XCloseIcon} onClick={() => quit()} />
			</div>
		</div>
	);
}

export default WindowFrame;
