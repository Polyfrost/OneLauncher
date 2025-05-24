import type React from 'react';
import type { RefAttributes } from 'react';
import type { SwitchProps as AriaSwitchProps } from 'react-aria-components';
import { Switch as AriaSwitch } from 'react-aria-components';
import { tv } from 'tailwind-variants';

export const switchVariants = tv({
	base: [
		'w-[40px] h-[22px] p-3 flex flex-row relative rounded-full transition-colors overflow-hidden bg-brand',
	],
})

export type SwitchProps = AriaSwitchProps & RefAttributes<HTMLLabelElement> & {
	className?: string;
    children?: React.ReactNode;
};

function Switch({
	className,
	...props
}: SwitchProps) {
	return (
		<AriaSwitch className={switchVariants({ className })} {...props}>
            <div className="bg-orange-400" />
            {props.children}
        </AriaSwitch>
	);
}

export default Switch;
