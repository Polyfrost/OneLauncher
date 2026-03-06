import { TextField } from '@onelauncher/common/components';
import { useState } from 'react';

type NumberSetting = [
	number,
	(value: number) => void,
];

export function SettingNumber({ setting, placeholder, min, max }: { setting: NumberSetting; placeholder?: string; min?: number; max?: number }) {
	const [inputValue, setInputValue] = useState(String(setting[0]));

	function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
		const value = event.currentTarget.value;
		setInputValue(value);
		const num = Number(value);
		if (!Number.isNaN(num))
			setting[1](num);
	}

	function handleBlur() {
		let num = Number(inputValue);

		if (Number.isNaN(num)) {
			setInputValue(String(setting[0]));
			return;
		}

		if (min !== undefined)
			num = Math.max(min, num);
		if (max !== undefined)
			num = Math.min(max, num);

		setInputValue(String(num));
		setting[1](num);
	}

	return (
		<TextField
			max={max}
			min={min}
			onBlur={handleBlur}
			onChange={handleChange}
			placeholder={placeholder}
			type="number"
			value={inputValue}
		/>
	);
}
