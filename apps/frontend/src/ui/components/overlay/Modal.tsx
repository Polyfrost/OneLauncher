import type {
	Context,
	JSX,
	ParentProps,
	Ref,
} from 'solid-js';
import { For, Show, createContext, createSignal, onCleanup, onMount, splitProps, useContext } from 'solid-js';
import { mergeRefs } from '@solid-primitives/refs';
import { createStore } from 'solid-js/store';
import { Transition } from 'solid-transition-group';
import Button from '../base/Button';
import FullscreenOverlay from './FullscreenOverlay';
import type { MakeOptional } from '~types.ts';

type ModalsList = (() => JSX.Element)[];

interface ModalContextType {
	modals: () => ModalsList;
	displayModal: (modal: () => JSX.Element) => number;
	closeModal: (index?: number) => void;
	isVisible: (index: number) => boolean;
};

const ModalContext = createContext() as Context<ModalContextType>;

export function ModalProvider(props: ParentProps) {
	const [modals, setModals] = createStore<ModalsList>([]);

	const context: ModalContextType = {
		modals: () => modals,

		displayModal: (modal) => {
			setModals(prev => [...prev, modal]);
			return modals.length - 1;
		},

		closeModal: (index?: number) => {
			if (modals.length === 0 || modals.length <= (index || 0))
				return;

			if (index !== undefined) {
				setModals(prev => prev.filter((_, i) => i !== index));
				return;
			}

			setModals(prev => prev.slice(0, -1));
		},

		isVisible: (index) => {
			return modals.length > index;
		},
	};

	return (
		<ModalContext.Provider value={context}>
			{props.children}
		</ModalContext.Provider>
	);
}

export function ModalRenderer() {
	const controller = useModalController();

	return (
		<FullscreenOverlay
			visible={() => controller.modals().length > 0}
			setVisible={(value) => {
				if (value === false)
					controller.closeModal();
			}}
		>
			<Transition
				mode="outin"
				enterClass="modal-animation-enter"
				enterActiveClass="modal-animation-enter-active"
				enterToClass="modal-animation-enter-to"
				exitClass="modal-animation-leave"
				exitActiveClass="modal-animation-leave-active"
				exitToClass="modal-animation-leave-to"
			>
				<Show when={controller.modals().length > 0}>
					{controller.modals()[controller.modals().length - 1]!()}
				</Show>
			</Transition>
		</FullscreenOverlay>
	);
}

export function useModalController() {
	const context = useContext(ModalContext);

	if (!context)
		throw new Error('useModal must be used within a ModalProvider');

	return context;
}

export interface ModalController {
	show: () => void;
	hide: () => void;
};

export function createModal(el: (props: ModalController) => JSX.Element): ModalController {
	const controller = useModalController();
	const [index, setIndex] = createSignal<number>();

	const ctx: ModalController = {
		show: () => {
			setIndex(controller.displayModal(() => el(ctx)));
		},
		hide: () => {
			controller.closeModal(index());
		},
	};

	return ctx;
}

export type ModalProps = {
	ref?: Ref<HTMLDivElement> | undefined;
} & ParentProps & ModalController;

function Modal(props: ModalProps) {
	return (
		<div ref={mergeRefs(props.ref)} class="bg-primary border border-white/5 p-4 rounded-lg text-center flex flex-col gap-y-2 min-w-xs">
			{props.children}
		</div>
	);
}

Modal.SplitProps = function <T extends ModalController>(props: T) {
	return splitProps(props, ['hide', 'show']);
};

type ModalButtonProps = string | JSX.Element;

export type ModalSimpleProps = {
	title: string;
	buttons?: ModalButtonProps[];
} & ModalProps;

Modal.Simple = function (props: ModalSimpleProps) {
	const [split, rest] = splitProps(props, ['title', 'buttons', 'children']);

	return (
		<Modal {...rest}>
			<h2>{split.title}</h2>
			<div class="flex flex-col">
				{split.children}
			</div>
			<div class="flex flex-row gap-x-4 [&>*]:flex-1 mt-2">
				<For each={split.buttons}>
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
	const [split, rest] = splitProps(props, ['title', 'buttons', 'onCancel', 'onDelete', 'timeLeft', 'children']);
	const [timeLeft, setTimeLeft] = createSignal(1);
	const [intervalId, setIntervalId] = createSignal<NodeJS.Timeout | undefined>(undefined);

	onMount(() => {
		startInterval(true);
	});

	onCleanup(() => {
		clearIntervalId();
	});

	function clearIntervalId() {
		if (intervalId())
			clearInterval(intervalId());
	}

	function startInterval(visible: boolean) {
		if (visible !== true)
			return;

		clearIntervalId();
		setTimeLeft(split.timeLeft || 3);

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
		if (split.onCancel)
			split.onCancel();

		props.hide();
	}

	function onDelete() {
		if (split.onDelete)
			split.onDelete();

		props.hide();
	}

	return (
		<Modal.Simple
			{...rest}
			title={split.title || 'Confirm Delete'}
			buttons={split.buttons || [
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
			{split.children || (
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
