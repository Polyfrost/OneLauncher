import type React from 'react';
import type { RefAttributes } from 'react';
import type { TooltipTriggerComponentProps as AriaTooltipTriggerProps } from 'react-aria-components';
import type { VariantProps } from 'tailwind-variants';
import {
	Tooltip as AriaTooltip,
	TooltipTrigger as AriaTooltipTrigger,
	OverlayArrow,
} from 'react-aria-components';
import { tv } from 'tailwind-variants';

export const tooltipVariants = tv({
	base: [
		'px-3 py-2 text-sm font-medium text-white bg-gray-900 rounded-lg shadow-lg',
		'border border-gray-700 max-w-xs z-50',
		'data-[entering]:animate-in data-[entering]:fade-in data-[entering]:zoom-in-95',
		'data-[exiting]:animate-out data-[exiting]:fade-out data-[exiting]:zoom-out-95',
	],
});

export const tooltipArrowVariants = tv({
	base: [
		'fill-gray-900 stroke-gray-700 stroke-1',
	],
});

export type TooltipVariantProps = VariantProps<typeof tooltipVariants>;
export type TooltipArrowVariantProps = VariantProps<typeof tooltipArrowVariants>;

export type TooltipProps = AriaTooltipTriggerProps & RefAttributes<HTMLElement> & {
	text: string;
	children: React.ReactElement;
	placement?: 'top' | 'bottom' | 'left' | 'right';
	className?: string;
	delay?: number;
	closeDelay?: number;
};

export function Tooltip({
	text,
	children,
	placement = 'top',
	className,
	delay = 700,
	closeDelay = 0,
	...props
}: TooltipProps) {
	return (
		<AriaTooltipTrigger closeDelay={closeDelay} delay={delay} {...props}>
			{children}
			<AriaTooltip className={tooltipVariants({ className })} placement={placement}>
				<OverlayArrow>
					<svg
						className={tooltipArrowVariants()}
						height={8}
						viewBox="0 0 8 8"
						width={8}
					>
						<path d="M0 0 L4 4 L8 0" />
					</svg>
				</OverlayArrow>
				{text}
			</AriaTooltip>
		</AriaTooltipTrigger>
	);
}
