import { HeartIcon, SearchMdIcon } from '@untitled-theme/icons-solid';
import { createSignal } from 'solid-js';
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

	const [list, key, setList, setKey] = createSortable<CardProps>([], {
		'A-Z': (a, b) => a.name.localeCompare(b.name),
		'Z-A': (a, b) => b.name.localeCompare(a.name),
		'Last Updated': (a, b) => a.date_updated - b.date_updated,
		'New': (a, b) => b.date_added - a.date_added,
		// 'Provider': (a, b) => a.provider
	});

	return (
		<div class="flex flex-col flex-grow-1">
			<div class="flex flex-row gap-2 h-8">
				<TextField iconLeft={<SearchMdIcon />} />
				<Dropdown class="w-32!">
					<Dropdown.Row>
						<HeartIcon />
						<p>Test</p>
					</Dropdown.Row>
					<Dropdown.Row>
						<HeartIcon />
						<p>Test 2</p>
					</Dropdown.Row>
					<Dropdown.Row>
						<p>Test 333333333</p>
					</Dropdown.Row>
					<Dropdown.Row>
						<HeartIcon />
						<p>Lorem Ipsum dolor</p>
					</Dropdown.Row>
				</Dropdown>
			</div>
		</div>
	);
}

export default BrowserMain;
