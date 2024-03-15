import {
    Accessor,
    ParentProps, Setter, Show, createEffect, createSignal,
} from 'solid-js';
import { Portal } from 'solid-js/web';

type ModalButton = {
    text: string,
    class?: string,
    onClick?: (event: MouseEvent) => void,
};

type ModalProps = {
    title: string,
    visible: Accessor<boolean>,
    setVisible: Setter<boolean>,
    buttons?: ModalButton[],
} & ParentProps;

function Modal(props: ModalProps) {
    const [animate, setAnimate] = createSignal('animate-fade-in');

    function onBackdropClick() {
        setAnimate('animate-fade-out pointer-events-none');
        setTimeout(() => {
            props.setVisible(false);
            setAnimate('animate-fade-in pointer-events-auto');
        }, 150);
    }

    return (
        <Show when={props.visible()}>
            <Portal>
                <div class={`fixed z-[1000] top-0 left-0 w-screen h-screen bg-black/50 backdrop-blur-md ${animate()}`}>
                    <div class='absolute w-full h-full' onClick={() => onBackdropClick()} />

                    <div class='absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2'>
                        <div class='bg-white p-4'>
                            <h1>Hello World</h1>
                        </div>
                    </div>

                </div>
            </Portal>
        </Show>
    );
}

export default Modal;
