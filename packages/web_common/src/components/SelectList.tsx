import type { JSX, ReactNode } from 'react';
import React, { createContext, useContext } from 'react';

interface SelectListContextType {
	onSelect: (value: string) => void;
	isSelected: (value: string) => boolean;
	multiple: boolean;
}

interface SelectListProps {
	value: string | Array<string>;
	onChange: (value: string | Array<string>) => void;
	className?: string;
	multiple?: boolean;
	children: JSX.Element;
}

interface SelectListItemProps {
	value: string;
	children: JSX.Element | any;
	className?: string;
}

const SelectListContext = createContext<SelectListContextType | null>(null);

export const SelectList: React.FC<SelectListProps> & { Item: React.FC<SelectListItemProps> } = ({
	value = '',
	onChange,
	className = '',
	multiple = false,
	children,
}) => {
	const handleSelect = (itemValue: string): void => {
		if (multiple) {
			const currentValues = Array.isArray(value) ? value : [];
			const newValues = currentValues.includes(itemValue)
				? currentValues.filter(v => v !== itemValue)
				: [...currentValues, itemValue];
			onChange(newValues);
		}
		else {
			onChange(itemValue);
		}
	};

	const isSelected = (itemValue: string): boolean => {
		if (multiple)
			return Array.isArray(value) && value.includes(itemValue);

		return value === itemValue;
	};

	const contextValue: SelectListContextType = {
		onSelect: handleSelect,
		isSelected,
		multiple,
	};

	return (
		<SelectListContext.Provider value={contextValue}>
			<div className={`border border-component-bg rounded-md ${className}`}>
				{children}
			</div>
		</SelectListContext.Provider>
	);
};

const SelectListItem: React.FC<SelectListItemProps> = ({ value, children, className = '' }) => {
	const context = useContext(SelectListContext);

	if (!context)
		throw new Error('SelectList.Item must be used within a SelectList');

	const { onSelect, isSelected } = context;
	const selected = isSelected(value);

	return (
		<div
			className={`cursor-pointer ${selected && 'bg-component-bg-pressed'} ${className}`}
			onClick={() => onSelect(value)}
		>
			{children}
		</div>
	);
};

SelectList.Item = SelectListItem;
