import type {
	Accessor,
	JSX,
	ParentProps,
	Ref,
	Setter,
} from 'solid-js';
import { For, createEffect, createSignal, on } from 'solid-js';
import { mergeRefs } from '@solid-primitives/refs';
import Button from '../base/Button';
import FullscreenOverlay from './FullscreenOverlay';
import type { MakeOptional } from '~types.ts';

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
			{/* <div ref={mergeRefs(props.ref)} class="absolute top-0 bottom-0 left-1/2 flex items-center"> */}
			<div ref={mergeRefs(props.ref)} class="bg-primary border border-white/5 p-4 rounded-lg text-center flex flex-col gap-y-2 min-w-xs">
				{props.children}
			</div>
			{/* </div> */}
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
			<div class="flex flex-row gap-x-4 [&>*]:flex-1 mt-2">
				<For each={props.buttons}>
					{button => (
						<ModalButton props={button} />
					)}
				</For>
			</div>
		</Modal>
	);
};

type ModalDeleteProps = MakeOptional<ModalSimpleProps, 'title'> & {
	onCancel?: () => void;
	onDelete?: () => void;
	timeLeft?: number;
};

Modal.Delete = function (props: ModalDeleteProps) {
	const [timeLeft, setTimeLeft] = createSignal(1);
	const [intervalId, setIntervalId] = createSignal<NodeJS.Timeout | undefined>(undefined);

	createEffect(on(() => props.visible(), (visible) => {
		startInterval(visible);
		if (visible !== true)
			onCancel();
	}));

	function clearIntervalId() {
		if (intervalId())
			clearInterval(intervalId());
	}

	function startInterval(visible: boolean) {
		if (visible !== true)
			return;

		clearIntervalId();
		setTimeLeft(props.timeLeft || 3);

		const intervalId = setInterval(() => {
			setTimeLeft((prev) => {
				const next = prev - 1;

				if (next <= 0)
					clearIntervalId();

				return next;
			});
		}, 1000);

		setIntervalId(intervalId);
	}

	function onCancel() {
		if (props.onCancel)
			props.onCancel();

		if (props.visible() === true)
			props.setVisible(false);
	}

	function onDelete() {
		if (props.onDelete)
			props.onDelete();

		if (props.visible() === true)
			props.setVisible(false);
	}

	return (
		<Modal.Simple
			title={props.title || 'Confirm Delete'}
			ref={props.ref}
			setVisible={props.setVisible}
			visible={props.visible}
			mount={props.mount}
			zIndex={props.zIndex}
			buttons={props.buttons || [
				<Button
					buttonStyle="secondary"
					children="Cancel"
					onClick={onCancel}
				/>,
				<Button
					buttonStyle="danger"
					children={`Delete${timeLeft() > 0 ? ` (${timeLeft()}s)` : ''}`}
					disabled={timeLeft() > 0}
					onClick={onDelete}
				/>,
			]}
		>
			{props.children || (
				<>
					<div class="flex flex-col justify-center items-center gap-y-3">
						<p>Are you sure you want to delete this item?</p>
						<p class="text-danger uppercase w-82 line-height-normal">
							Doing this will
							{' '}
							<span class="underline font-bold">delete</span>
							{' '}
							your entire
							{' '}
							<br />
							data
							{' '}
							<span class="underline font-bold">FOREVER</span>
							.
						</p>
					</div>
				</>
			)}
		</Modal.Simple>
	);
};

function ModalButton(button: { props: ModalButtonProps }) {
	return (
		<>
			{typeof button.props === 'string'
				? (
						<Button buttonStyle="secondary">
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
