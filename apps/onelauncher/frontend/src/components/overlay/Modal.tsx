import type { ClassNameString } from '@onelauncher/common';
import type { DialogProps } from 'react-aria-components';
import {
	Dialog as AriaDialog,
	DialogTrigger,
	Modal as ModalOverlay,
} from 'react-aria-components';

interface ModalProps extends DialogProps, ClassNameString { }

function Modal({ className, ...props }: ModalProps) {
	return (
		<ModalOverlay className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4" isDismissable>
			<AriaDialog className="" {...props} />
		</ModalOverlay>
	);
}

Modal.Trigger = DialogTrigger;

export default Modal;
