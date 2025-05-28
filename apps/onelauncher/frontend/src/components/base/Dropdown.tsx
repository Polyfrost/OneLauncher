import type { ClassNameString } from '@/types/global';
import type { 
    SelectProps as AriaComboBoxProps, 
    ListBoxItemProps as AriaListBoxItemProps
} from 'react-aria-components';
import { 
    Select as AriaComboBox,
    Label as AriaLabel,
    ListBox as AriaListBox,
    ListBoxItem as AriaListBoxItem,
    Popover as AriaPopover, 
    SelectValue
} from 'react-aria-components';
import { tv } from 'tailwind-variants';
import Button from './Button';

interface ComboBoxWrapperProps<T extends object> extends Omit<AriaComboBoxProps<T>, 'className'>, ClassNameString {
    label?: string;
}

const dropdownStyles = tv({
    base: [
        "flex flex-col border border-component-bg-hover w-fit rounded-lg focus:outline-none"
    ]
})

function Dropdown<T extends object>({
    className,
    label,
    children,
    ...rest
}: ComboBoxWrapperProps<T>) {
    return (
        <AriaComboBox
            className={dropdownStyles({ className })}
            {...rest}
        >
            {label && <AriaLabel>{label}</AriaLabel>}
            <div className="flex">
                <SelectValue />
                <Button className="size-6">▼</Button>
            </div>
            <AriaPopover className="border border-orange-400">
                <AriaListBox className="bg-blue-500 p-2 w-38">
                    {children}
                </AriaListBox>
            </AriaPopover>
        </AriaComboBox>
    );
}

export default Dropdown;

Dropdown.Item = ({
    className,
    ...rest
}: AriaListBoxItemProps & ClassNameString) => (
    <AriaListBoxItem
        className={className}
        {...rest}
    />
);