// //! Account container that checks ownership on deserialization.

// use crate::bpf_writer::BpfWriter;
// // use crate::error::{Error, ErrorCode};
// use crate::{
//     AccountDeserialize, AccountSerialize, AccountsClose, AccountsExit, Key, Owner,
//     Result, ToAccountInfo, ToAccountInfos, ToAccountMetas, CompressedState,
// };
// use solana_program::account_info::AccountInfo;
// use solana_program::hash::hash;
// use solana_program::instruction::AccountMeta;
// use solana_program::pubkey::Pubkey;
// // use solana_program::system_program;
// // use std::collections::{BTreeMap, BTreeSet};
// use std::fmt;
// use std::io::Write;
// use std::ops::{Deref, DerefMut};

// /// Wrapper around [`AccountInfo`](crate::solana_program::account_info::AccountInfo)
// /// that verifies program ownership, deserializes state, enforced state parity and matches state data into a Rust type.

// #[derive(Clone)]
// pub struct CompressedAccount<'info, T: AccountSerialize + AccountDeserialize + Clone> {
//     account: T,
//     state: CompressedState,
//     info: AccountInfo<'info>,
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug> fmt::Debug
//     for CompressedAccount<'info, T>
// {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         self.fmt_with_name("CompressedAccount", f)
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug> CompressedAccount<'info, T> {
//     pub(crate) fn fmt_with_name(&self, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_struct(name)
//             .field("account", &self.account)
//             .field("state", &self.state)
//             .field("info", &self.info)
//             .finish()
//     }
// }

// impl<'a, T: AccountSerialize + AccountDeserialize + Clone> CompressedAccount<'a, T> {
//     // pub(crate) fn new(info: AccountInfo<'a>, account: T, state: CompressedState) -> CompressedAccount<'a, T> {
//     //     Self { info, account, state }
//     // }

//     pub(crate) fn exit_with_expected_owner(
//         &self,
//         expected_owner: &Pubkey,
//         program_id: &Pubkey,
//     ) -> Result<()> {
//         // Only persist if the owner is the current program and the account is not closed.
//         if expected_owner == program_id && !crate::common::is_closed(&self.info) {
//             let info = self.to_account_info();
//             let mut data = info.try_borrow_mut_data()?;
//             let dst: &mut [u8] = &mut data;
//             let mut state_data = vec![];
//             self.account.try_serialize(&mut state_data)?;
//             // TODO: Emit a noop here
//             let hash = hash(&state_data).to_bytes();
//             let mut writer = BpfWriter::new(dst);
//             writer.write(&hash)?;
//         }
//         Ok(())
//     }

//     /// Reloads the state from storage. This is useful, for example, when
//     /// observing side effects after CPI.
//     pub fn reload(&mut self) -> Result<()> {
//         let mut data: &[u8] = &self.info.try_borrow_data()?;
//         self.state = CompressedState::try_from(&mut data)?;
//         Ok(())
//     }

//     pub fn into_inner(self) -> T {
//         self.account
//     }

//     /// Sets the inner account.
//     ///
//     /// Instead of this:
//     /// ```ignore
//     /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
//     ///     (*ctx.accounts.user_to_create).name = new_user.name;
//     ///     (*ctx.accounts.user_to_create).age = new_user.age;
//     ///     (*ctx.accounts.user_to_create).address = new_user.address;
//     /// }
//     /// ```
//     /// You can do this:
//     /// ```ignore
//     /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
//     ///     ctx.accounts.user_to_create.set_inner(new_user);
//     /// }
//     /// ```
//     pub fn set_inner(&mut self, inner: T) {
//         self.account = inner;
//     }
// }

// // impl<'a, T: AccountSerialize + AccountDeserialize + Owner + Clone> CompressedAccount<'a, T> {
// //     /// Deserializes the given `info` into a `CompressedAccount`.
// //     #[inline(never)]
// //     pub fn try_from_state(info: &AccountInfo<'a>, state: &mut &[u8]) -> Result<CompressedAccount<'a, T>> {
// //         if info.owner == &system_program::ID && info.lamports() == 0 {
// //             return Err(ErrorCode::AccountNotInitialized.into());
// //         }
// //         if info.owner != &T::owner() {
// //             return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
// //                 .with_pubkeys((*info.owner, T::owner())));
// //         }

// //         let compressed_state = CompressedState::try_from(&info.try_borrow_data()?)?;
// //         compressed_state.verify_state(state)?;

// //         Ok(CompressedAccount::new(info.clone(), T::try_deserialize(state)?, compressed_state))
// //     }

// //     /// Deserializes the given `info` into a `CompressedAccount` without checking
// //     /// the account discriminator. Be careful when using this and avoid it if
// //     /// possible.
// //     #[inline(never)]
// //     pub fn try_from_state_unchecked(info: &AccountInfo<'a>, state: &mut &[u8]) -> Result<CompressedAccount<'a, T>> {
// //         if info.owner == &system_program::ID && info.lamports() == 0 {
// //             return Err(ErrorCode::AccountNotInitialized.into());
// //         }
// //         if info.owner != &T::owner() {
// //             return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
// //                 .with_pubkeys((*info.owner, T::owner())));
// //         }

// //         let compressed_state = CompressedState::try_from(&info.try_borrow_data()?)?;
// //         compressed_state.verify_state(state)?;
        
// //         Ok(CompressedAccount::new(info.clone(), T::try_deserialize_unchecked(state)?, compressed_state))
// //     }
// // }

// // impl<'info, T: AccountSerialize + AccountDeserialize + Owner + Clone> Accounts<'info>
// //     for CompressedAccount<'info, T>
// // where
// //     T: AccountSerialize + AccountDeserialize + Owner + Clone,
// // {
// //     #[inline(never)]
// //     fn try_accounts(
// //         _program_id: &Pubkey,
// //         accounts: &mut &[AccountInfo<'info>],
// //         _ix_data: &[u8],
// //         _bumps: &mut BTreeMap<String, u8>,
// //         _seeds: &mut BTreeMap<String, Vec<u8>>,
// //         state: &mut BTreeMap<String, Vec<u8>>,
// //         _reallocs: &mut BTreeSet<Pubkey>,
// //     ) -> Result<Self> {
// //         if accounts.is_empty() {
// //             return Err(ErrorCode::AccountNotEnoughKeys.into());
// //         }
// //         let account = &accounts[0];
// //         *accounts = &accounts[1..];
// //         CompressedAccount::try_from_state(account)
// //     }
// // }

// impl<'info, T: AccountSerialize + AccountDeserialize + Owner + Clone> AccountsExit<'info>
//     for CompressedAccount<'info, T>
// {
//     fn exit(&self, program_id: &Pubkey) -> Result<()> {
//         self.exit_with_expected_owner(&T::owner(), program_id)
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AccountsClose<'info>
//     for CompressedAccount<'info, T>
// {
//     fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
//         crate::common::close(self.to_account_info(), sol_destination)
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> ToAccountMetas for CompressedAccount<'info, T> {
//     fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
//         let is_signer = is_signer.unwrap_or(self.info.is_signer);
//         let meta = match self.info.is_writable {
//             false => AccountMeta::new_readonly(*self.info.key, is_signer),
//             true => AccountMeta::new(*self.info.key, is_signer),
//         };
//         vec![meta]
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> ToAccountInfos<'info>
//     for CompressedAccount<'info, T>
// {
//     fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
//         vec![self.info.clone()]
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AsRef<AccountInfo<'info>>
//     for CompressedAccount<'info, T>
// {
//     fn as_ref(&self) -> &AccountInfo<'info> {
//         &self.info
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AsRef<T> for CompressedAccount<'info, T> {
//     fn as_ref(&self) -> &T {
//         &self.account
//     }
// }

// impl<'a, T: AccountSerialize + AccountDeserialize + Clone> Deref for CompressedAccount<'a, T> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         &(self).account
//     }
// }

// impl<'a, T: AccountSerialize + AccountDeserialize + Clone> DerefMut for CompressedAccount<'a, T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         #[cfg(feature = "anchor-debug")]
//         if !self.info.is_writable {
//             solana_program::msg!("The given Account is not mutable");
//             panic!();
//         }
//         &mut self.account
//     }
// }

// impl<'info, T: AccountSerialize + AccountDeserialize + Clone> Key for CompressedAccount<'info, T> {
//     fn key(&self) -> Pubkey {
//         *self.info.key
//     }
// }
