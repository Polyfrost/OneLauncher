import { SearchMdIcon } from '@untitled-theme/icons-solid';
import { For, createSignal } from 'solid-js';
import Dropdown from '~ui/components/base/Dropdown';
import TextField from '~ui/components/base/TextField';
import createSortable from '~utils/sorting';

interface CardProps {
	id: string;
	name: string;
	date_added: number;
	date_updated: number;
	provider: Provider;
};

enum Provider {
	Curseforge,
	Modrinth,
	Polyfrost,
	Skyclient,
}

interface BrowserFilters {
	provider: Provider[]; // TODO: Change with Provider types from Rust
}

function BrowserMain() {
	const [filters, setFilters] = createSignal<BrowserFilters>({
		provider: [Provider.Curseforge],
	});

	const sortable = createSortable<CardProps>([], {
		'A-Z': (a, b) => a.name.localeCompare(b.name),
		'Z-A': (a, b) => b.name.localeCompare(a.name),
		'Last Updated': (a, b) => a.date_updated - b.date_updated,
		'New': (a, b) => b.date_added - a.date_added,
	});

	function onSortChange(selected: number) {
		sortable.setKey(selected);
	}

	return (
		<div class="flex flex-col flex-grow-1">
			<div class="flex flex-row gap-2 h-8">
				<TextField iconLeft={<SearchMdIcon />} />
				<Dropdown onChange={onSortChange} text="Sort By: ">
					<For each={Object.keys(sortable.sortables)}>
						{sortable => (
							<Dropdown.Row>{sortable}</Dropdown.Row>
						)}
					</For>
				</Dropdown>
			</div>
		</div>
	);
}

export default BrowserMain;
