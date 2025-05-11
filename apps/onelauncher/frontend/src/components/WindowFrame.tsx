import type { ComponentType, JSX } from 'react';
import type { PressEvent } from 'react-aria-components';
import { Window } from '@tauri-apps/api/window';
import { ChevronLeftIcon, Maximize02Icon, Minimize01Icon, MinusIcon, XCloseIcon } from '@untitled-theme/icons-react';
import { useEffect, useState } from 'react';
import { twMerge } from 'tailwind-merge';
import Button from './base/Button';

function WindowFrame() {
	const [isMaximized, setIsMaximized] = useState(false);

	const back = () => history.back();
	const minimize = () => Window.getCurrent().minimize();
	const toggleMaximize = () => Window.getCurrent().toggleMaximize();
	const quit = () => Window.getCurrent().close();

	useEffect(() => {
		let unlisten: (() => void) | undefined;

		(async () => {
			unlisten = await Window.getCurrent().onResized(() => {
				Window.getCurrent().isMaximized().then(setIsMaximized);
			});
		})();

		return () => unlisten?.();
	}, []);

	return (
		<div className="z-[99999] w-screen flex flex-row items-center justify-between gap-0.5 bg-window-frame" data-tauri-drag-region>
			<div className="flex-1 flex flex-row items-center pointer-events-none">
				<TitlebarButton icon={ChevronLeftIcon} onClick={back} />
			</div>

			<div className="flex-1 flex flex-row items-center justify-center pointer-events-none">
				<span className="text-base text-center">OneLauncher</span>
			</div>

			<div className="flex-1 flex flex-row items-center justify-end pointer-events-none">
				<TitlebarButton icon={MinusIcon} onClick={minimize} />
				<TitlebarButton icon={isMaximized ? Minimize01Icon : Maximize02Icon} onClick={toggleMaximize} />
				<TitlebarButton danger icon={XCloseIcon} onClick={quit} />
			</div>
		</div>
	);
}

export default WindowFrame;

function TitlebarButton({
	icon: Icon,
	onClick,
	danger = false,
}: {
	icon: ComponentType<JSX.IntrinsicElements['svg']>;
	onClick: (e: PressEvent) => void;
	danger?: boolean;
}) {
	return (
		<Button
			className="h-full aspect-square p-1 group flex items-center justify-center pointer-events-auto hover:bg-transparent"
			color="ghost"
			onPress={onClick}
			size="icon"
		>
			<div className={twMerge('rounded-lg h-full aspect-square flex justify-center items-center', danger ? 'group-hover:bg-danger-hover' : 'group-hover:bg-component-bg-hover')}>
				<Icon className="w-5 h-5 stroke-fg-primary group-hover:stroke-fg-primary-hover" />
			</div>
		</Button>
	);
}
