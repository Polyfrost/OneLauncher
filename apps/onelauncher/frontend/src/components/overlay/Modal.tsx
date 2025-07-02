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
		<ModalOverlay isDismissable>
			<AriaDialog className="" {...props} />

		</ModalOverlay>
	);
}

Modal.Trigger = DialogTrigger;

export default Modal;
