import { Window } from '@tauri-apps/api/window';
import { MinusIcon, XCloseIcon } from '@untitled-theme/icons-solid';
import { JSX, createSignal } from 'solid-js';
import Modal from '../overlay/Modal';

type ButtonProps = {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    icon: (any: any) => JSX.Element,
    onClick: (event: MouseEvent) => void,
    danger?: boolean,
};

function Button(props: ButtonProps) {
    return (
        <button class='flex items-center justify-center w-8 h-8 group' onClick={(e) => props.onClick(e)}>
            <div class='rounded-lg group-hover:bg-bg-primary p-1'>
                <props.icon class={`w-[18px] h-[18px] stroke-slate ${props.danger ? 'group-hover:stroke-danger' : 'group-hover:stroke-white'}`} />
            </div>
        </button>
    );
}

function WindowFrame() {
    const [isCloseModalVisible, setIsCloseModalVisible] = createSignal(false);

    return (
        <div data-tauri-drag-region class="flex flex-row gap-2 justify-end items-center w-screen h-8 bg-bg-secondary pr-4">
            <Button icon={MinusIcon} onClick={() => Window.getCurrent().minimize()} />
            <Button icon={XCloseIcon} onClick={() => {
                setIsCloseModalVisible(!isCloseModalVisible());
            }} danger />

            <Modal title='Close' visible={isCloseModalVisible} setVisible={setIsCloseModalVisible} />
        </div>
    );
}

export default WindowFrame;
