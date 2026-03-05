import { TextField } from '@onelauncher/common/components';

type NumberSetting = [
	number,
	(value: number) => void,
];

export function SettingNumber({ setting, placeholder, min, max }: { setting: NumberSetting; placeholder?: string; min?: number; max?: number }) {
	function handleChange(event: React.ChangeEvent<HTMLInputElement>) {
		let value = Number(event.currentTarget.value);

		if (Number.isNaN(value))
			return;

		if (min !== undefined)
			value = Math.max(min, value);
		if (max !== undefined)
			value = Math.min(max, value);

		setting[1](value);
	}

	return (
		<TextField
			max={max}
			min={min}
			onChange={handleChange}
			placeholder={placeholder}
			type="number"
			value={setting[0]}
		/>
	);
}
