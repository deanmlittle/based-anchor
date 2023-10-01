//! Account container that checks ownership on deserialization.

use crate::bpf_writer::BpfWriter;
use crate::error::{Error, ErrorCode};
use crate::{
    AccountDeserialize, AccountSerialize, Accounts, AccountsClose, AccountsExit, Key, Owner,
    Result, ToAccountInfo, ToAccountInfos, ToAccountMetas, Migrate,
};
use solana_program::account_info::AccountInfo;
use solana_program::instruction::AccountMeta;
use solana_program::pubkey::Pubkey;
use solana_program::system_program;
use std::collections::{BTreeMap, BTreeSet};
use std::{fmt, marker::PhantomData};
use std::ops::{Deref, DerefMut};


/// Wrapper around [`AccountInfo`](crate::solana_program::account_info::AccountInfo)
/// focused on handling and assisting with migrations within Solana's Anchor framework.
/// 
/// # Table of Contents
/// - [How Migration Works](#how-migration-works)
/// - [Reallocating After Migration](#reallocating-after-migration)
///
/// # How Migration Works
///
/// In Anchor, state is a specialized type of account that can store global data. 
/// However, the structure of the state may need adjustments over time as programs evolve, necessitating state migrations.
/// State migrations are necessary when one needs to shift from one type of state structure to another. 
/// In practical terms, you might be introducing new fields, adjusting existing ones, or eliminating some of them.
///
/// ## Example of a State Migration in Anchor
/// 
/// The migration process in Based Anchor involves:
/// - Defining the new state structure.
/// - Implementing the `Migrate` trait to outline how the data from the old state maps to the new state.
/// - Using the migration function to transition from the old state to the new one.
///
/// ```rust
/// #[account]
/// pub struct TypeA {
///     pub bump: u8,
///     pub data: Vec<u8>
/// }
/// 
/// impl Space for TypeA {
///     const INIT_SPACE: usize = 8 + 1 + 4 + 32;
/// }
///
/// #[account]
/// pub struct TypeB {
///     pub bump: u8,
///     pub data: Vec<u8>
/// }
/// 
/// impl Space for TypeB  {
///     const INIT_SPACE: usize = 8 + 1 + 4 + 64;
/// }
///
/// impl Migrate<TypeB> for TypeA {
///     fn migrate(&self) -> TypeB {
///         TypeB {
///             bump: self.bump,
///             data: vec![0x0b; 64]
///         }
///     }
/// }
/// ```
///
/// - Structural Redefinition: The code initially defines two state structures, `TypeA` and `TypeB`. `TypeB` appears to be an expanded version of `TypeA`, showcasing a common scenario in migrations.
/// - Migration Logic: Implement the `Migrate` trait for `TypeA`, indicating how it should transition to `TypeB`. Here, the data from `TypeA` is taken and expanded upon to match the format of `TypeB`.
/// - Invoking Migration: When you wish to perform the migration, your program will load `TypeA`, execute the migrate function, and then save the resulting `TypeB` state. The intricacies of this process, such as reallocation of storage, are handled internally by Anchor.
/// 
/// 
/// # Reallocating After Migration
///
/// Once the state migration has been successfully executed, it's often necessary to reallocate memory to fit the new state structure. 
/// In Solana, accounts have a fixed size once initialized. So, as your state struct changes, its size can vary. 
/// For example, if you introduce new fields or expand existing ones, the state might require more space. 
/// Conversely, if you're streamlining your state or removing unnecessary fields, you might free up space.
/// 
/// Reallocating ensures that:
/// - You aren't wasting space on the Solana blockchain (and thus not incurring unnecessary costs).
/// - Your migration can operate without errors due to memory constraints.
/// 
/// ## Example of how Reallocation Works in Migration
/// Let's continue to examine the accounts of the previous example to see how reallocation works.
/// 
/// ```rust
/// #[derive(Accounts)]
/// pub struct MigrateBigToSmall<'info> {
///     #[account(
///         mut,
///         seeds = [b"account", signer.key().as_ref()],
///         bump = data.bump,
///         realloc = TypeA::INIT_SPACE,
///         realloc::zero = false, 
///         realloc::payer = signer
///     )]
///     data: Migration<'info, TypeB, TypeA>,
///     system_program: Program<'info, System>
/// }
/// 
/// ```
/// In the migration context `MigrateBigToSmall`, notice the `realloc` attribute. 
/// It indicates the new space requirement for the state after migration, defined by `TypeA::INIT_SPACE`.
/// 
/// The attributes under `realloc` include:
/// - `realloc::zero`: Whether or not to zero out the additional memory. Here, it's set to `false`.
/// - `realloc::payer`: Specifies who will bear the cost of reallocation. In this case, the `signer` account covers the costs.



#[derive(Clone)]

pub struct Migration<'info, MigrateFrom, MigrateTo>
where
    MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>,
    MigrateTo: AccountSerialize + Clone,
{
    account: MigrateFrom,
    info: AccountInfo<'info>,
    _phantom: PhantomData<MigrateTo>,
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo> + fmt::Debug, MigrateTo: AccountSerialize + Clone + fmt::Debug> fmt::Debug
    for Migration<'info, MigrateFrom, MigrateTo>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_name("Migration", f)
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo> + fmt::Debug, MigrateTo: AccountSerialize + Clone + fmt::Debug> Migration<'info, MigrateFrom, MigrateTo> {
    pub(crate) fn fmt_with_name(&self, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("account", &self.account)
            .field("info", &self.info)
            .finish()
    }
}

impl<'a, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> Migration<'a, MigrateFrom, MigrateTo> {
    pub(crate) fn new(info: AccountInfo<'a>, account: MigrateFrom) -> Migration<'a, MigrateFrom, MigrateTo> {
        Self { info, account, _phantom: PhantomData }
    }

    pub(crate) fn exit_with_expected_owner(
        &self,
        expected_owner: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<()> {
        // Only persist if the owner is the current program and the account is not closed.
        if expected_owner == program_id && !crate::common::is_closed(&self.info) {
            let info = self.to_account_info();
            let mut data = info.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut writer = BpfWriter::new(dst);
            self.account.migrate().try_serialize(&mut writer)?;
        }
        Ok(())
    }

    /// Reloads the account from storage. This is useful, for example, when
    /// observing side effects after CPI.
    pub fn reload(&mut self) -> Result<()> {
        let mut data: &[u8] = &self.info.try_borrow_data()?;
        self.account = MigrateFrom::try_deserialize(&mut data)?;
        Ok(())
    }

    pub fn into_inner(self) -> MigrateFrom {
        self.account
    }

    /// Sets the inner account.
    ///
    /// Instead of this:
    /// ```ignore
    /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
    ///     (*ctx.accounts.user_to_create).name = new_user.name;
    ///     (*ctx.accounts.user_to_create).age = new_user.age;
    ///     (*ctx.accounts.user_to_create).address = new_user.address;
    /// }
    /// ```
    /// You can do this:
    /// ```ignore
    /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
    ///     ctx.accounts.user_to_create.set_inner(new_user);
    /// }
    /// ```
    pub fn set_inner(&mut self, inner: MigrateFrom) {
        self.account = inner;
    }
}

impl<'a, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo> + Owner, MigrateTo: AccountSerialize + Clone> Migration<'a, MigrateFrom, MigrateTo> {
    /// Deserializes the given `info` into a `Account`.
    #[inline(never)]
    pub fn try_from(info: &AccountInfo<'a>) -> Result<Migration<'a, MigrateFrom, MigrateTo>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        if info.owner != &MigrateFrom::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, MigrateFrom::owner())));
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Migration::new(info.clone(), MigrateFrom::try_deserialize(&mut data)?))
    }

    /// Deserializes the given `info` into a `Account` without checking
    /// the account discriminator. Be careful when using this and avoid it if
    /// possible.
    #[inline(never)]
    pub fn try_from_unchecked(info: &AccountInfo<'a>) -> Result<Migration<'a, MigrateFrom, MigrateTo>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        if info.owner != &MigrateFrom::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, MigrateFrom::owner())));
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Migration::new(
            info.clone(),
            MigrateFrom::try_deserialize_unchecked(&mut data)?
        ))
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo> + Owner, MigrateTo: AccountSerialize + Clone> Accounts<'info>
    for Migration<'info, MigrateFrom, MigrateTo>
where
    MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, 
    MigrateTo: AccountSerialize + Clone
{
    #[inline(never)]
    fn try_accounts(
        _program_id: &Pubkey,
        accounts: &mut &[AccountInfo<'info>],
        _ix_data: &[u8],
        _bumps: &mut BTreeMap<String, u8>,
        _reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        if accounts.is_empty() {
            return Err(ErrorCode::AccountNotEnoughKeys.into());
        }
        let account = &accounts[0];
        *accounts = &accounts[1..];
        Migration::try_from(account)
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo> + Owner, MigrateTo: AccountSerialize + Clone> AccountsExit<'info>
    for Migration<'info, MigrateFrom, MigrateTo>
{
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        self.exit_with_expected_owner(&MigrateFrom::owner(), program_id)
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> AccountsClose<'info>
    for Migration<'info, MigrateFrom, MigrateTo>
{
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        crate::common::close(self.to_account_info(), sol_destination)
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> ToAccountMetas for Migration<'info, MigrateFrom, MigrateTo> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> ToAccountInfos<'info>
    for Migration<'info, MigrateFrom, MigrateTo>
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> AsRef<AccountInfo<'info>>
    for Migration<'info, MigrateFrom, MigrateTo>
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        &self.info
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> AsRef<MigrateFrom> for Migration<'info, MigrateFrom, MigrateTo> {
    fn as_ref(&self) -> &MigrateFrom {
        &self.account
    }
}

impl<'a, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> Deref for Migration<'a, MigrateFrom, MigrateTo> {
    type Target = MigrateFrom;

    fn deref(&self) -> &Self::Target {
        &(self).account
    }
}

impl<'a, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> DerefMut for Migration<'a, MigrateFrom, MigrateTo> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(feature = "anchor-debug")]
        if !self.info.is_writable {
            solana_program::msg!("The given Migration is not mutable");
            panic!();
        }
        &mut self.account
    }
}

impl<'info, MigrateFrom: AccountDeserialize + Clone + Migrate<MigrateTo>, MigrateTo: AccountSerialize + Clone> Key for Migration<'info, MigrateFrom, MigrateTo> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
