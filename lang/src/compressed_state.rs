use crate::{AnchorSerialize, AnchorDeserialize};
use solana_program::hash::hash;
use crate::Result;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub struct CompressedState {
    version: CompressedStateVersion,
    state: Vec<u8>
}

// By having a compressed state version header, we can expand our parsing logic in the future.
// This means we could potentially enable extremely fast iterations on things like merkle trees,
// storing multiple state hashes in a single account, or a whole host of other things.
#[repr(u8)]
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, Debug)]
pub enum CompressedStateVersion {
    Zero = 0,
    Hash = 1
}

impl TryFrom<&u8> for CompressedStateVersion {
    type Error = anchor_lang::error::Error;
    fn try_from(value: &u8) -> Result<Self> {
        match value {
            0 => Ok(CompressedStateVersion::Zero),
            1 => Ok(CompressedStateVersion::Hash),
            _ => Err(anchor_lang::error::ErrorCode::CompressedStateInvalidVersion.into())
        }
    }
}

impl CompressedState {
    pub fn try_from(data: &[u8]) -> Result<Self> {
        let version = match data.get(0) {
            Some(n) => CompressedStateVersion::try_from(n)?,
            None => return Err(anchor_lang::error::ErrorCode::CompressedStateDidNotDeserialize.into())
        };

        match version {
            CompressedStateVersion::Zero => {
                Ok(Self {
                        version,
                        state: vec![]
                })
            },
            CompressedStateVersion::Hash => {
                let state = match data.get(1..33) {
                    Some(h) => h.to_vec(),
                    None => return Err(anchor_lang::error::ErrorCode::CompressedStateDidNotDeserialize.into())
                };
                Ok(Self {
                    version,
                    state
                })
            }
        }
    }

    pub fn verify_state(&self, state: &[u8]) -> Result<()> {
        match self.version {
            CompressedStateVersion::Zero => Ok(()),
            CompressedStateVersion::Hash => match &hash(state).as_ref() == &self.state {
                true => Ok(()),
                false => Err(anchor_lang::error::ErrorCode::CompressedStateMismatch.into())
            }
        }
    }
}