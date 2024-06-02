import type {
	Accessor,
	JSX,
	ParentProps,
	Setter,
} from 'solid-js';
import { For, mergeProps } from 'solid-js';
import Button from '../base/Button';
import FullscreenOverlay from './FullscreenOverlay';

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
	const merged: ModalProps = mergeProps({ buttons: ['Ok'] }, props);

	return (
		<FullscreenOverlay
			visible={props.visible}
			setVisible={props.setVisible}
		>
			<div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2">
				<div class="bg-primary border border-white/5 p-4 rounded-lg text-center flex flex-col gap-y-2 min-w-xs">
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
		</FullscreenOverlay>
	);
}

export default Modal;
