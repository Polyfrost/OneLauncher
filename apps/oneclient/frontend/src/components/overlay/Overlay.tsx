import { Button } from '@onelauncher/common/components';
import { XCloseIcon } from '@untitled-theme/icons-react';
import { Dialog, Modal, ModalOverlay } from 'react-aria-components';
import { twMerge } from 'tailwind-merge';
import { tv } from 'tailwind-variants';

const modalVariants = tv({
	slots: {
		overlay: [
			'fixed inset-0 z-40',
			'flex items-center justify-center',
			'bg-black/50 p-4',

			'entering:animate-duration-150 entering:animate-fade',
			'exiting:animate-duration-100 exiting:animate-fade exiting:animate-reverse',
		],
		modal: [
			'entering:animate-duration-150 entering:animate-ease-in-out entering:animate-zoom',
			'exiting:animate-duration-100 exiting:animate-ease-in-out exiting:animate-zoom exiting:animate-reverse',
		],
		dialog: [
			'flex flex-col items-center min-w-sm min-h-32 p-4 gap-4 relative',
			'border border-component-border bg-page-elevated rounded-2xl',
		],
	},
});

export type OverlayProps = React.ComponentProps<typeof ModalOverlay> & {
	className?: string | undefined;
	children?: React.ReactNode | undefined;
};

export function Overlay({
	className,
	children,
	...rest
}: OverlayProps) {
	const { overlay, modal } = modalVariants();

	return (
		<ModalOverlay
			className={twMerge(overlay(), className)}
			isDismissable
			{...rest}
		>
			<Modal className={modal()}>
				{children}
			</Modal>
			<p className="fixed bottom-0 left-1/2 -translate-x-1/2 text-xs text-fg-secondary text-center py-2 pointer-events-none">Click outside to dismiss</p>
		</ModalOverlay>
	);
}

Overlay.Dialog = function OverlayDialog({
	className,
	children,
}: {
	className?: string | undefined;
	children?: React.ReactNode | undefined;
}) {
	const { dialog } = modalVariants();

	return (
		<Dialog className={twMerge(dialog(), className)} role="dialog">
			<div className="absolute top-5.5 right-5.5">
				<Button color="ghost" size="icon" slot="close">
					<XCloseIcon />
				</Button>
			</div>

			{children}
		</Dialog>
	);
};

Overlay.Title = function OverlayTitle({ children }: { children: React.ReactNode }) {
	return <h2 className="text-2xl font-semibold">{children}</h2>;
};
