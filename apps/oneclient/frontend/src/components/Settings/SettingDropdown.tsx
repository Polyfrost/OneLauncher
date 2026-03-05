import { TitleCase } from '@/utils/string';
import { Dropdown } from '@onelauncher/common/components';

export function SettingDropdown<T extends string>({ setting, options }: { setting: [T, (value: T) => void]; options: Array<T> }) {
	return (
		<Dropdown onSelectionChange={key => setting[1](key as T)} selectedKey={setting[0]}>
			{options.map(option => <Dropdown.Item id={option} key={option}>{TitleCase(option)}</Dropdown.Item>)}
		</Dropdown>
	);
}
