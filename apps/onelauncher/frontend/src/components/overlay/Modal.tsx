import { Dialog, DialogTrigger, Heading, Input, Label, Modal as AriaModal, TextField, ModalOverlay } from 'react-aria-components';
import Button from '../base/Button';

// just testing stuff
export function Modal() {
	return (
		<>
			<DialogTrigger>
				<Button className={"focus:outline-none"}>Slmlar</Button>
				<ModalOverlay isDismissable className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
					<AriaModal>
						<Dialog className="min-w-md flex flex-col gap-y-2 border border-white/5 rounded-lg bg-page p-4 text-center focus:outline-none">
							<form>
								<Heading slot="title">Sign up</Heading>
								<TextField autoFocus>
									<Label>First Name:</Label>
									<Input />
								</TextField>
								<TextField>
									<Label>Last Name:</Label>
									<Input />
								</TextField>
								<Button slot="close">
									Submit
								</Button>
							</form>
						</Dialog>
					</AriaModal>
				</ModalOverlay>
			</DialogTrigger>
		</>
	)
}