import { LogOut01Icon, PlusIcon, Settings01Icon } from '@untitled-theme/icons-solid';
import Popup from './Popup';
import Button from '../base/Button';

type AccountComponentProps = {
    username: string,
    headSrc: string,
    loggedIn?: boolean,
};

function AccountComponent(props: AccountComponentProps) {
    return (
        <div class={`flex flex-row justify-between p-2 rounded-lg ${!props.loggedIn && 'hover:bg-gray-.5 active:bg-gray-.10 hover:text-fg-primary-hover'}`}>
            <div class='flex flex-row justify-start flex-1 gap-x-3'>
                <img class='w-8 h-8 rounded-md' src={props.headSrc} alt="" />
                <div class='flex flex-col items-center justify-center'>
                    <div class='flex flex-col items-start justify-between'>
                        <p class='font-semibold h-[18px]'>{props.username}</p>
                        {props.loggedIn && <p class='text-xs'>Logged in</p>}
                    </div>
                </div>
            </div>
            {props.loggedIn && (
                <Button styleType='icon' class='w-8 h-8'>
                    <LogOut01Icon class=' stroke-danger' />
                </Button>
            )}
        </div>
    );
}

function AccountPopup(props: Popup.PopupProps) {
    return (
        <Popup {...props}>
            <div class='bg-component-bg rounded-xl border border-gray-.10 w-72 p-2 shadow-lg shadow-black/50'>
                <div class='flex flex-col gap-y-2 text-fg-primary'>
                    <AccountComponent username='Caledonian' headSrc='https://crafatar.com/avatars/f247be7c5b8241c69148793ded77e71f?size=100' loggedIn />

                    <div class='w-full h-px bg-gray-.5 rounded-md' />

                    <AccountComponent username='Caledonian' headSrc='https://crafatar.com/avatars/f247be7c5b8241c69148793ded77e71f?size=100' />
                    <AccountComponent username='Caledonian' headSrc='https://crafatar.com/avatars/f247be7c5b8241c69148793ded77e71f?size=100' />
                    <AccountComponent username='Caledonian' headSrc='https://crafatar.com/avatars/f247be7c5b8241c69148793ded77e71f?size=100' />

                    <div class='w-full h-px bg-gray-.5 rounded-md' />

                    <div class='flex flex-row justify-between'>
                        <div>
                            <Button styleType='ghost' iconLeft={<PlusIcon />}>Add Account</Button>
                        </div>
                        <div class='flex flex-row'>
                            <Button styleType='icon' class='w-9 h-9'>
                                <Settings01Icon class='p-0.5 stroke-fg-primary' />
                            </Button>
                        </div>
                    </div>

                </div>
            </div>
        </Popup>
    );
}

export default AccountPopup;
