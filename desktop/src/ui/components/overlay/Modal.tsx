import type {
	Accessor,
	JSX,
	ParentProps,
	Setter,
} from 'solid-js';
import { For, Show, createEffect, createSignal, mergeProps } from 'solid-js';
import { Portal } from 'solid-js/web';
import Button from '../base/Button';

type ModalButtonProps = string | JSX.Element;

function ModalButton(button: { props: ModalButtonProps }) {
	return (
		<>
			{typeof button.props === 'string'
				? (
					<Button>
						{button.props}
					</Button>
					)
				: (
						button.props
					)}
		</>
	);
}

type ModalProps = {
	title: string;
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
	buttons?: ModalButtonProps[];
} & ParentProps;

function Modal(props: ModalProps) {
	const [localVisible, setLocalVisible] = createSignal(false);
	const [animate, setAnimate] = createSignal('animate-fade-in');
	const merged: ModalProps = mergeProps({ buttons: ['Ok'] }, props);

	createEffect(() => {
		if (props.visible()) {
			setAnimate('animate-fade-in');
			setLocalVisible(true);
		}
		else {
			setAnimate('animate-fade-out');
			setTimeout(() => {
				setLocalVisible(false);
			}, 150);
		}
	});

	function onBackdropClick() {
		props.setVisible(false);
	}

	return (
		<Show when={localVisible()}>
			<Portal>
				<div class={`fixed z-[1000] top-0 left-0 w-screen h-screen bg-black/60 backdrop-blur-sm backdrop-grayscale ${animate()}`}>
					<div class="absolute w-full h-full" onClick={() => onBackdropClick()} />

					<div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
						<div class="bg-primary border border-white/5 p-4 rounded-lg text-center flex flex-col gap-y-2">
							<h2>{props.title}</h2>
							<div class="flex flex-col">
								{props.children}
							</div>
							<div class="flex flex-row gap-x-4 [&>*]:flex-1">
								<For each={merged.buttons}>
									{button => (
										<ModalButton props={button} />
									)}
								</For>
							</div>
						</div>
					</div>

				</div>
			</Portal>
		</Show>
	);
}

export default Modal;
