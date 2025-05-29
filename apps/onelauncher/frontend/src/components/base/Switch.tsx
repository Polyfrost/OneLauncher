import type React from 'react';
import type { RefAttributes } from 'react';
import type { SwitchProps as AriaSwitchProps } from 'react-aria-components';
import { Switch as AriaSwitch } from 'react-aria-components';
import { tv } from 'tailwind-variants';

export const switchVariants = tv({
    base: [
        'group inline-flex h-6 w-11 items-center rounded-full transition-colors',
        'bg-component-bg-pressed data-[selected]:bg-blue-600',
        'focus:outline-none',
        'disabled:cursor-not-allowed disabled:opacity-50',
    ],
});

export const switchThumbVariants = tv({
    base: [
        'pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow ring-0 transition-transform',
        'translate-x-1 group-data-[selected]:translate-x-6',
    ],
});

export type SwitchProps = AriaSwitchProps & RefAttributes<HTMLLabelElement> & {
    className?: string;
    children?: React.ReactNode;
};

function Switch({
    className,
    children,
    ...props
}: SwitchProps) {
    return (
        <AriaSwitch className={switchVariants({ className })} {...props}>
            <span className={switchThumbVariants()} />
            {children && <span className="ml-3 text-sm font-medium text-gray-900">{children}</span>}
        </AriaSwitch>
    );
}

export default Switch;