import { TextField } from '@onelauncher/common/components';

type StringSetting = [
	string,
	(value: string) => void,
];

export function SettingString({ setting, placeholder }: { setting: StringSetting; placeholder?: string }) {
	return (
		<TextField
			onChange={event => setting[1](event.currentTarget.value)}
			placeholder={placeholder}
			value={setting[0]}
		/>
	);
}
