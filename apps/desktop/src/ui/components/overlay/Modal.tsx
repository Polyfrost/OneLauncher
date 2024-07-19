import type {
	Accessor,
	JSX,
	ParentProps,
	Ref,
	Setter,
} from 'solid-js';
import { For } from 'solid-js';
import { mergeRefs } from '@solid-primitives/refs';
import Button from '../base/Button';
import FullscreenOverlay from './FullscreenOverlay';

export type ModalProps = {
	visible: Accessor<boolean>;
	setVisible: Setter<boolean>;
	zIndex?: number | undefined;
	mount?: Node | undefined;
	ref?: Ref<HTMLDivElement> | undefined;
} & ParentProps;

// TODO: Implement some sort of stacking control?
function Modal(props: ModalProps) {
	return (
		<FullscreenOverlay
			visible={props.visible}
			setVisible={props.setVisible}
			mount={props.mount}
			zIndex={props.zIndex || 1000}
		>
			<div ref={mergeRefs(props.ref)} class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
				<div class="bg-primary border border-white/5 p-4 rounded-lg text-center flex flex-col gap-y-2 min-w-xs">
					{props.children}
				</div>
			</div>
		</FullscreenOverlay>
	);
}

type ModalButtonProps = string | JSX.Element;

export type ModalSimpleProps = {
	title: string;
	buttons?: ModalButtonProps[];
} & ModalProps;

Modal.Simple = function (props: ModalSimpleProps) {
	return (
		<Modal
			ref={props.ref}
			setVisible={props.setVisible}
			visible={props.visible}
			mount={props.mount}
			zIndex={props.zIndex}
		>
			<h2>{props.title}</h2>
			<div class="flex flex-col">
				{props.children}
			</div>
			<div class="flex flex-row gap-x-4 [&>*]:flex-1">
				<For each={props.buttons}>
					{button => (
						<ModalButton props={button} />
					)}
				</For>
			</div>
		</Modal>
	);
};

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

export default Modal;
