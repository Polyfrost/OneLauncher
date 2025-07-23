import type { DialogProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import {
	Dialog as AriaDialog,
	Modal as AriaModal,
	DialogTrigger,
	ModalOverlay,
} from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { tv } from 'tailwind-variants';

const modalVariants = tv({
	slots: {
		overlay: [
			'fixed inset-0 z-50',
			'flex items-center justify-center',
			'bg-black/50 p-4',

			'data-[entering]:animate-in data-[entering]:fade-in data-[entering]:duration-200',
			'data-[exiting]:animate-out data-[exiting]:fade-out data-[exiting]:duration-150',
		],
		modal: [
			'data-[entering]:animate-in data-[entering]:zoom-in-95 data-[entering]:duration-200',
			'data-[exiting]:animate-out data-[exiting]:zoom-out-95 data-[exiting]:duration-150',
		],
		dialog: [
			'outline-none',
		],
	},
});

export interface ModalProps extends DialogProps, VariantProps<typeof modalVariants> {
	isOpen?: boolean;
	onOpenChange?: (isOpen: boolean) => void;
	isDismissable?: boolean;
	overlayClassName?: string;
	modalClassName?: string;
}

function Modal({
	className,
	isOpen,
	onOpenChange,
	isDismissable = true,
	overlayClassName,
	modalClassName,
	...props
}: ModalProps) {
	const { overlay, modal, dialog } = modalVariants();

	const isControlled = isOpen !== undefined || onOpenChange !== undefined;

	const overlayProps = {
		className: twMerge(overlay(), overlayClassName),
		isDismissable,
		...(isControlled && { isOpen, onOpenChange }),
	};

	return (
		<ModalOverlay {...overlayProps}>
			<AriaModal className={twMerge(modal(), modalClassName)}>
				<AriaDialog
					className={twMerge(dialog(), className)}
					{...props}
				/>
			</AriaModal>
		</ModalOverlay>
	);
}

Modal.Trigger = DialogTrigger;

export default Modal;
