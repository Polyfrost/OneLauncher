import { Button, Show } from '@onelauncher/common/components';
import { ChevronRightIcon, ChevronLeftIcon } from '@untitled-theme/icons-react';
import { useState } from 'react';

export interface PaginationOptions {
	itemsCount: number;
	itemsPerPage: number;
	initialPage?: number;
};

function usePagination(options: PaginationOptions) {
	// const [options, setOptions] = useState(initialOptions);

	const [page, setPage] = useState(Math.max(1, options.initialPage || 1));

	const next = () => setPage(page => page + 1);
	const prev = () => setPage(page => page - 1);

	const totalPages =  Math.ceil(options.itemsCount / options.itemsPerPage);

	const hasNext = page < totalPages;
	const hasPrev = page > 1;

	const offset = (page - 1) * options.itemsPerPage;

	// Exported
	function reset(/*options: PaginationOptions*/) {
		setPage(1);
	}

	function getVisiblePageNumbers(page: number) {
		const pages: number[] = [];

		const left = page - 1 > 1 ? page - 1 : null;
		const right = page + 1 < totalPages ? page + 1 : null;

		if (left !== null)
			pages.push(left);

		if (page !== 1 && page !== totalPages)
			pages.push(page);

		if (right !== null)
			pages.push(right);

		return pages;
	}

	function PaginationBtn(props: { page: number }) {
		return (
			<Button
				className="p-2 text-lg min-w-8"
				color={props.page === page ? 'primary' : 'secondary'}

				onClick={() => {
					if (props.page === page)
						return;

					setPage(props.page);
				}}
			>
				<span className='text-trim'>{props.page}</span>
			</Button>
		);
	}

	const Navigation = () => (
		<div className="flex flex-row items-center justify-center gap-x-1">

			<Button
				size='icon'
				color='secondary'
				children={<ChevronLeftIcon />}
				isDisabled={page <= 1}
				onClick={() => setPage(page => page - 1)}
			/>

			<PaginationBtn page={1} />

			<Show when={page > 1 + 2}>
				<span>...</span>
			</Show>

			{getVisiblePageNumbers(page).map(page=><PaginationBtn page={page} />)}

			<Show when={page < totalPages - 2}>
				<span>...</span>
			</Show>

			<Show when={totalPages > 1}>
				<PaginationBtn page={totalPages} />
			</Show>

			<Button
				size="icon"
				color='secondary'
				children={<ChevronRightIcon />}
				isDisabled={page >= totalPages}
				onClick={() => setPage(page => page + 1)}
			/>
		</div>
	);

	return {
		page,
		next,
		prev,
		hasNext,
		hasPrev,
		totalPages,
		offset,
		reset,
		options,
		Navigation,
	};
}

export default usePagination;
