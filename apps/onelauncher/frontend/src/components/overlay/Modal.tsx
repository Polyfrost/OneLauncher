import type { ClassNameString } from '@/types/global';
import {
  Dialog as AriaDialog,
  DialogTrigger,
  Modal as ModalOverlay
} from 'react-aria-components';
import type { DialogProps } from 'react-aria-components';

interface ModalProps extends DialogProps, ClassNameString { }

function Modal({ className, ...props }: ModalProps) {
  return (
    <ModalOverlay isDismissable className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <AriaDialog className="min-w-md flex flex-col gap-y-2 border border-white/5 rounded-lg bg-page p-4 text-center focus:outline-none" {...props} />
    </ModalOverlay>
  );
}

Modal.Trigger = DialogTrigger;

export default Modal;
