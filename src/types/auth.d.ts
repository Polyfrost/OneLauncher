declare namespace auth {
    export interface Account {
        username: string,
        uuid: string,
        skins: AccountSkin[]
    }

    export interface AccountSkin {
        id: string,
        state: 'active' | 'something',
        url: string,
        variant: 'CLASSIC' | 'SLIM'
    }
}
