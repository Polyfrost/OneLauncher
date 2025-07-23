import type { ComponentType, SVGProps } from 'react';
import type { DialogProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import { Button } from '@onelauncher/common/components';
import { ArrowRightIcon } from '@untitled-theme/icons-react';
import { createElement } from 'react';
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

export default function Modal({
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

export interface ModalHeaderProps {
	name: string;
	fontsize?: number;
	icon?: ComponentType<SVGProps<SVGSVGElement>>;
	banner?: string;
	currentStep?: {
		id: string;
		title: string;
	};
}

Modal.Header = ({ name, banner, fontsize, icon, currentStep }: ModalHeaderProps) => (
	<div className="theme-OneLauncher-Dark relative h-25 flex">
		{banner
			? (
					<div className="absolute left-0 top-0 h-full w-full">
						<img
							alt="Header Image"
							className="h-full w-full rounded-t-lg"
							src={banner}
						/>
					</div>
				)
			: null}
		<div className="absolute left-0 top-0 h-full flex w-full flex-row items-center justify-start gap-x-4 bg-[radial-gradient(at_center,#00000077,transparent)] px-10">
			{icon && createElement(icon, { className: 'h-8 w-8 text-fg-primary' })}
			<div className="flex flex-col items-start justify-center">
				<h1 className={`h-10 text-fg-primary text-[${fontsize ?? '32'}px] font-semibold`}>{name}</h1>
				{currentStep ? <span className="text-fg-primary">{currentStep.title}</span> : null}
			</div>
		</div>
	</div>
);

export interface ModalFooterProps {
	isFirstStep: boolean;
	isNextDisabled: boolean;
	nextButtonText: string;
	onBack: () => void;
	onNext: () => void;
}

Modal.Footer = ({
	isFirstStep,
	isNextDisabled,
	nextButtonText,
	onBack,
	onNext,
}: ModalFooterProps) => (
	<div className="flex flex-row justify-end gap-x-2 p-3 pt-0">
		<Button
			color="ghost"
			isDisabled={isFirstStep}
			onClick={onBack}
		>
			Previous
		</Button>
		<Button
			color="primary"
			isDisabled={isNextDisabled}
			onClick={onNext}
		>
			{nextButtonText}
			{' '}
			<ArrowRightIcon />
		</Button>
	</div>
);
