// Smoldot
// Copyright (C) 2019-2022  Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Data structures containing the finalized state of the chain, except for its storage.
//!
//! The types provided in this module contain the state of the chain, other than its storage, that
//! has been finalized.
//!
//! > **Note**: These data structures only provide a way to communicate that finalized state, but
//! >           the existence of a [`ChainInformation`] alone does in no way mean that its content
//! >           is accurate. As an example, one use case of [`ChainInformation`] is to be written
//! >           to disk then later reloaded. It is possible for the user to modify the data on
//! >           disk, in which case the loaded [`ChainInformation`] might be erroneous.
//!
//! These data structures contain all the information that is necessary to verify the
//! authenticity (but not the correctness) of blocks that descend from the finalized block
//! contained in the structure.
//!
//! They do not, however, contain the storage of the finalized block, which is necessary to verify
//! the correctness of new blocks. It is possible, though, for instance to download the
//! storage of the finalized block from another node. This downloaded storage can be verified
//! to make sure that it matches the content of the [`ChainInformation`].
//!
//! They also do not contain the past history of the chain. It is, however, similarly possible to
//! for instance download the history from other nodes.

use crate::header;

use alloc::{boxed::Box, vec::Vec};
use core::num::NonZero;

pub mod build;

/// Information about the latest finalized block and state found in its ancestors.
///
/// Similar to [`ChainInformation`], but guaranteed to be coherent.
#[derive(Debug, Clone)]
pub struct ValidChainInformation {
    inner: ChainInformation,
}

impl From<ValidChainInformation> for ChainInformation {
    fn from(i: ValidChainInformation) -> Self {
        i.inner
    }
}

impl ValidChainInformation {
    /// Gives access to the information.
    pub fn as_ref(&self) -> ChainInformationRef {
        From::from(&self.inner)
    }
}

impl<'a> From<ValidChainInformationRef<'a>> for ValidChainInformation {
    fn from(info: ValidChainInformationRef<'a>) -> ValidChainInformation {
        ValidChainInformation {
            inner: info.inner.into(),
        }
    }
}

impl TryFrom<ChainInformation> for ValidChainInformation {
    type Error = ValidityError;

    fn try_from(info: ChainInformation) -> Result<Self, Self::Error> {
        ChainInformationRef::from(&info).validate()?;
        Ok(ValidChainInformation { inner: info })
    }
}

/// Information about the latest finalized block and state found in its ancestors.
///
/// Similar to [`ChainInformationRef`], but guaranteed to be coherent.
#[derive(Debug, Clone)]
pub struct ValidChainInformationRef<'a> {
    inner: ChainInformationRef<'a>,
}

impl<'a> From<&'a ValidChainInformation> for ValidChainInformationRef<'a> {
    fn from(info: &'a ValidChainInformation) -> ValidChainInformationRef<'a> {
        ValidChainInformationRef {
            inner: From::from(&info.inner),
        }
    }
}

impl<'a> TryFrom<ChainInformationRef<'a>> for ValidChainInformationRef<'a> {
    type Error = ValidityError;

    fn try_from(info: ChainInformationRef<'a>) -> Result<Self, Self::Error> {
        info.validate()?;
        Ok(ValidChainInformationRef { inner: info })
    }
}

impl<'a> ValidChainInformationRef<'a> {
    /// Gives access to the information.
    pub fn as_ref(&self) -> ChainInformationRef<'a> {
        self.inner.clone()
    }
}

/// Information about the latest finalized block and state found in its ancestors.
#[derive(Debug, Clone)]
pub struct ChainInformation {
    /// Header of the highest known finalized block.
    pub finalized_block_header: Box<header::Header>,

    /// Extra items that depend on the consensus engine.
    pub consensus: ChainInformationConsensus,

    /// Extra items that depend on the finality engine.
    pub finality: ChainInformationFinality,
}

impl<'a> From<ChainInformationRef<'a>> for ChainInformation {
    fn from(info: ChainInformationRef<'a>) -> ChainInformation {
        ChainInformation {
            finalized_block_header: Box::new(info.finalized_block_header.into()),
            consensus: match info.consensus {
                ChainInformationConsensusRef::Unknown => ChainInformationConsensus::Unknown,
                ChainInformationConsensusRef::Aura {
                    finalized_authorities_list,
                    slot_duration,
                } => ChainInformationConsensus::Aura {
                    finalized_authorities_list: finalized_authorities_list
                        .map(|a| a.into())
                        .collect(),
                    slot_duration,
                },
                ChainInformationConsensusRef::Babe {
                    slots_per_epoch,
                    finalized_next_epoch_transition,
                    finalized_block_epoch_information,
                } => ChainInformationConsensus::Babe {
                    slots_per_epoch,
                    finalized_block_epoch_information: finalized_block_epoch_information
                        .map(|i| Box::new(i.into())),
                    finalized_next_epoch_transition: Box::new(
                        finalized_next_epoch_transition.into(),
                    ),
                },
            },
            finality: info.finality.into(),
        }
    }
}

/// Extra items that depend on the consensus engine.
#[derive(Debug, Clone)]
pub enum ChainInformationConsensus {
    /// Any node on the chain is allowed to produce blocks.
    ///
    /// > **Note**: Be warned that this variant makes it possible for a huge number of blocks to
    /// >           be produced. If this variant is used, the user is encouraged to limit, through
    /// >           other means, the number of blocks being accepted.
    Unknown,

    /// Chain is using the Aura consensus engine.
    Aura {
        /// List of authorities that must validate children of the block referred to by
        /// [`ChainInformation::finalized_block_header`].
        finalized_authorities_list: Vec<header::AuraAuthority>,

        /// Duration, in milliseconds, of an Aura slot.
        slot_duration: NonZero<u64>,
    },

    /// Chain is using the Babe consensus engine.
    Babe {
        /// Number of slots per epoch. Configured at the genesis block and never touched later.
        slots_per_epoch: NonZero<u64>,

        /// Babe epoch information about the epoch the finalized block belongs to.
        ///
        /// If the finalized block belongs to epoch #0, which starts at block #1, then this must
        /// contain the information about the epoch #0, which can be found by calling the
        /// `BabeApi_configuration` runtime function.
        ///
        /// Must be `None` if and only if the finalized block is block #0.
        ///
        /// > **Note**: The information about the epoch the finalized block belongs to isn't
        /// >           necessary, but the information about the epoch the children of the
        /// >           finalized block belongs to *is*. However, due to possibility of missed
        /// >           slots, it is often not possible to know in advance whether the children
        /// >           of a block will belong to the same epoch as their parent. This is the
        /// >           reason why the "parent" (i.e. finalized block)'s information are demanded.
        finalized_block_epoch_information: Option<Box<BabeEpochInformation>>,

        /// Babe epoch information about the epoch right after the one the finalized block belongs
        /// to.
        ///
        /// If [`ChainInformationConsensus::Babe::finalized_block_epoch_information`] is `Some`,
        /// this field must contain the epoch that follows.
        ///
        /// If the finalized block is block #0, then this must contain the information about the
        /// epoch #0, which can be found by calling the `BabeApi_configuration` runtime function.
        finalized_next_epoch_transition: Box<BabeEpochInformation>,
    },
}

/// Information about a Babe epoch.
#[derive(Debug, Clone)]
pub struct BabeEpochInformation {
    /// Index of the epoch.
    ///
    /// Epoch number 0 starts at the slot number of block 1. Epoch indices increase one by one.
    pub epoch_index: u64,

    /// Slot at which the epoch starts.
    ///
    /// Must be `None` if and only if the context is
    /// [`ChainInformationConsensus::Babe::finalized_next_epoch_transition`] and
    /// [`BabeEpochInformation::epoch_index`] is 0.
    pub start_slot_number: Option<u64>,

    /// List of authorities allowed to author blocks during this epoch.
    pub authorities: Vec<header::BabeAuthority>,

    /// Randomness value for this epoch.
    ///
    /// Determined using the VRF output of the validators of the epoch before.
    pub randomness: [u8; 32],

    /// Value of the constant that allows determining the chances of a VRF being generated by a
    /// given slot.
    ///
    /// This constant represents a fraction, where the first element of the tuple is the numerator
    /// and the second element is the denominator. The fraction should always be `<= 1`, meaning
    /// that the numerator should always be inferior or equal to the denominator.
    pub c: (u64, u64),

    /// Types of blocks allowed for this epoch.
    pub allowed_slots: header::BabeAllowedSlots,
}

impl BabeEpochInformation {
    /// Checks whether the fields in this struct make sense.
    pub fn validate(&self) -> Result<(), BabeValidityError> {
        BabeEpochInformationRef::from(self).validate()
    }
}

impl<'a> From<BabeEpochInformationRef<'a>> for BabeEpochInformation {
    fn from(info: BabeEpochInformationRef<'a>) -> BabeEpochInformation {
        BabeEpochInformation {
            epoch_index: info.epoch_index,
            start_slot_number: info.start_slot_number,
            authorities: info.authorities.map(Into::into).collect(),
            randomness: *info.randomness,
            c: info.c,
            allowed_slots: info.allowed_slots,
        }
    }
}

/// Extra items that depend on the finality engine.
#[derive(Debug, Clone)]
pub enum ChainInformationFinality {
    /// Blocks themselves don't contain any information concerning finality. Finality is provided
    /// by a mechanism that is entirely external to the chain.
    ///
    /// > **Note**: This is the mechanism used for parachains. Finality is provided entirely by
    /// >           the relay chain.
    Outsourced,

    /// Chain uses the Grandpa finality algorithm.
    Grandpa {
        /// Grandpa authorities set ID of the block right after finalized block.
        ///
        /// If the finalized block is the genesis block, should be 0. Otherwise, must be
        /// incremented by one for every change in the Grandpa authorities reported by the
        /// headers since the genesis block.
        after_finalized_block_authorities_set_id: u64,

        /// List of GrandPa authorities that need to finalize the block right after the finalized
        /// block.
        finalized_triggered_authorities: Vec<header::GrandpaAuthority>,

        /// Change in the GrandPa authorities list that has been scheduled by a block that is already
        /// finalized, but the change is not triggered yet. These changes will for sure happen.
        /// Contains the block number where the changes are to be triggered.
        ///
        /// The block whose height is contained in this field must still be finalized using the
        /// authorities found in [`ChainInformationFinality::Grandpa::finalized_triggered_authorities`].
        /// Only the next block and further use the new list of authorities.
        ///
        /// The block height must always be strictly superior to the height found in
        /// [`ChainInformation::finalized_block_header`].
        ///
        /// > **Note**: When a header contains a GrandPa scheduled changes log item with a delay of N,
        /// >           the block where the changes are triggered is
        /// >           `height(block_with_log_item) + N`. If `N` is 0, then the block where the
        /// >           change is triggered is the same as the one where it is scheduled.
        finalized_scheduled_change: Option<(u64, Vec<header::GrandpaAuthority>)>,
    },
}

impl<'a> From<ChainInformationFinalityRef<'a>> for ChainInformationFinality {
    fn from(finality: ChainInformationFinalityRef<'a>) -> ChainInformationFinality {
        match finality {
            ChainInformationFinalityRef::Outsourced => ChainInformationFinality::Outsourced,
            ChainInformationFinalityRef::Grandpa {
                after_finalized_block_authorities_set_id,
                finalized_triggered_authorities,
                finalized_scheduled_change,
            } => ChainInformationFinality::Grandpa {
                after_finalized_block_authorities_set_id,
                finalized_scheduled_change: finalized_scheduled_change.map(|(n, l)| (n, l.into())),
                finalized_triggered_authorities: finalized_triggered_authorities.into(),
            },
        }
    }
}

/// Equivalent to a [`ChainInformation`] but referencing an existing structure. Cheap to copy.
#[derive(Debug, Clone)]
pub struct ChainInformationRef<'a> {
    /// See equivalent field in [`ChainInformation`].
    pub finalized_block_header: header::HeaderRef<'a>,

    /// Extra items that depend on the consensus engine.
    pub consensus: ChainInformationConsensusRef<'a>,

    /// Extra items that depend on the finality engine.
    pub finality: ChainInformationFinalityRef<'a>,
}

impl<'a> ChainInformationRef<'a> {
    /// Checks whether the information is coherent.
    pub fn validate(&self) -> Result<(), ValidityError> {
        if let ChainInformationConsensusRef::Babe {
            finalized_next_epoch_transition,
            finalized_block_epoch_information,
            ..
        } = &self.consensus
        {
            if let Err(err) = finalized_next_epoch_transition.validate() {
                return Err(ValidityError::InvalidBabe(err));
            }

            if finalized_next_epoch_transition.start_slot_number.is_some()
                && (finalized_next_epoch_transition.epoch_index == 0)
            {
                return Err(ValidityError::UnexpectedBabeSlotStartNumber);
            }
            if finalized_next_epoch_transition.start_slot_number.is_none()
                && (finalized_next_epoch_transition.epoch_index != 0)
            {
                return Err(ValidityError::MissingBabeSlotStartNumber);
            }

            if let Some(finalized_block_epoch_information) = &finalized_block_epoch_information {
                if let Err(err) = finalized_block_epoch_information.validate() {
                    return Err(ValidityError::InvalidBabe(err));
                }

                if self.finalized_block_header.number == 0 {
                    return Err(ValidityError::UnexpectedBabeFinalizedEpoch);
                }

                if let Some(epoch_start_slot_number) =
                    finalized_block_epoch_information.start_slot_number
                {
                    if let Some(babe_preruntime) =
                        self.finalized_block_header.digest.babe_pre_runtime()
                    {
                        if self.finalized_block_header.number == 0 {
                            return Err(ValidityError::ConsensusAlgorithmMismatch);
                        }
                        if babe_preruntime.slot_number() < epoch_start_slot_number {
                            return Err(ValidityError::HeaderBabeSlotInferiorToEpochStartSlot);
                        }
                    } else if self.finalized_block_header.number != 0 {
                        return Err(ValidityError::ConsensusAlgorithmMismatch);
                    }
                    if (self.finalized_block_header.digest.babe_seal().is_some()
                        != (self.finalized_block_header.number != 0))
                        || self.finalized_block_header.digest.has_any_aura()
                    {
                        return Err(ValidityError::ConsensusAlgorithmMismatch);
                    }
                    if let Some((epoch_change, _new_config)) =
                        self.finalized_block_header.digest.babe_epoch_information()
                    {
                        if epoch_change.authorities != finalized_next_epoch_transition.authorities
                            || epoch_change.randomness != finalized_next_epoch_transition.randomness
                        {
                            return Err(ValidityError::BabeEpochInfoMismatch);
                        }
                    }
                } else {
                    return Err(ValidityError::MissingBabeSlotStartNumber);
                }
            }

            if finalized_block_epoch_information.is_none()
                && self.finalized_block_header.number != 0
            {
                return Err(ValidityError::NoBabeFinalizedEpoch);
            }
        }

        if let ChainInformationConsensusRef::Aura { .. } = &self.consensus {
            if (self
                .finalized_block_header
                .digest
                .aura_pre_runtime()
                .is_some()
                != (self.finalized_block_header.number != 0))
                || (self.finalized_block_header.digest.aura_seal().is_some()
                    != (self.finalized_block_header.number != 0))
                || self.finalized_block_header.digest.has_any_babe()
            {
                return Err(ValidityError::ConsensusAlgorithmMismatch);
            }
        }

        if let ChainInformationFinalityRef::Grandpa {
            after_finalized_block_authorities_set_id,
            finalized_scheduled_change,
            ..
        } = &self.finality
        {
            // TODO: check consistency with the finalized block header
            if let Some(change) = finalized_scheduled_change.as_ref() {
                if change.0 <= self.finalized_block_header.number {
                    return Err(ValidityError::ScheduledGrandPaChangeBeforeFinalized);
                }
            }
            if self.finalized_block_header.number == 0
                && *after_finalized_block_authorities_set_id != 0
            {
                return Err(ValidityError::FinalizedZeroButNonZeroAuthoritiesSetId);
            }
        }

        Ok(())
    }
}

impl<'a> From<&'a ChainInformation> for ChainInformationRef<'a> {
    fn from(info: &'a ChainInformation) -> ChainInformationRef<'a> {
        ChainInformationRef {
            finalized_block_header: (&*info.finalized_block_header).into(),
            consensus: match &info.consensus {
                ChainInformationConsensus::Unknown => ChainInformationConsensusRef::Unknown,
                ChainInformationConsensus::Aura {
                    finalized_authorities_list,
                    slot_duration,
                } => ChainInformationConsensusRef::Aura {
                    finalized_authorities_list: header::AuraAuthoritiesIter::from_slice(
                        finalized_authorities_list,
                    ),
                    slot_duration: *slot_duration,
                },
                ChainInformationConsensus::Babe {
                    slots_per_epoch,
                    finalized_block_epoch_information,
                    finalized_next_epoch_transition,
                } => ChainInformationConsensusRef::Babe {
                    slots_per_epoch: *slots_per_epoch,
                    finalized_block_epoch_information: finalized_block_epoch_information
                        .as_ref()
                        .map(|i| (&**i).into()),
                    finalized_next_epoch_transition: (&**finalized_next_epoch_transition).into(),
                },
            },
            finality: (&info.finality).into(),
        }
    }
}

/// Extra items that depend on the consensus engine.
#[derive(Debug, Clone)]
pub enum ChainInformationConsensusRef<'a> {
    /// See [`ChainInformationConsensus::Unknown`].
    Unknown,

    /// Chain is using the Aura consensus engine.
    Aura {
        /// See equivalent field in [`ChainInformationConsensus`].
        finalized_authorities_list: header::AuraAuthoritiesIter<'a>,

        /// See equivalent field in [`ChainInformationConsensus`].
        slot_duration: NonZero<u64>,
    },

    /// Chain is using the Babe consensus engine.
    Babe {
        /// See equivalent field in [`ChainInformationConsensus`].
        slots_per_epoch: NonZero<u64>,

        /// See equivalent field in [`ChainInformationConsensus`].
        finalized_block_epoch_information: Option<BabeEpochInformationRef<'a>>,

        /// See equivalent field in [`ChainInformationConsensus`].
        finalized_next_epoch_transition: BabeEpochInformationRef<'a>,
    },
}

/// Information about a Babe epoch.
#[derive(Debug, Clone)]
pub struct BabeEpochInformationRef<'a> {
    /// See equivalent field in [`BabeEpochInformation`].
    pub epoch_index: u64,

    /// See equivalent field in [`BabeEpochInformation`].
    pub start_slot_number: Option<u64>,

    /// See equivalent field in [`BabeEpochInformation`].
    pub authorities: header::BabeAuthoritiesIter<'a>,

    /// See equivalent field in [`BabeEpochInformation`].
    pub randomness: &'a [u8; 32],

    /// See equivalent field in [`BabeEpochInformation`].
    pub c: (u64, u64),

    /// See equivalent field in [`BabeEpochInformation`].
    pub allowed_slots: header::BabeAllowedSlots,
}

impl<'a> BabeEpochInformationRef<'a> {
    /// Checks whether the fields in this struct make sense.
    pub fn validate(&self) -> Result<(), BabeValidityError> {
        if self.c.0 > self.c.1 {
            return Err(BabeValidityError::InvalidConstant);
        }

        Ok(())
    }
}

impl<'a> From<&'a BabeEpochInformation> for BabeEpochInformationRef<'a> {
    fn from(info: &'a BabeEpochInformation) -> BabeEpochInformationRef<'a> {
        BabeEpochInformationRef {
            epoch_index: info.epoch_index,
            start_slot_number: info.start_slot_number,
            authorities: header::BabeAuthoritiesIter::from_slice(&info.authorities),
            randomness: &info.randomness,
            c: info.c,
            allowed_slots: info.allowed_slots,
        }
    }
}

/// Extra items that depend on the finality engine.
#[derive(Debug, Clone)]
pub enum ChainInformationFinalityRef<'a> {
    /// See equivalent variant in [`ChainInformationFinality`].
    Outsourced,

    /// See equivalent variant in [`ChainInformationFinality`].
    Grandpa {
        /// See equivalent field in [`ChainInformationFinality`].
        after_finalized_block_authorities_set_id: u64,

        /// See equivalent field in [`ChainInformationFinality`].
        finalized_triggered_authorities: &'a [header::GrandpaAuthority],

        /// See equivalent field in [`ChainInformationFinality`].
        finalized_scheduled_change: Option<(u64, &'a [header::GrandpaAuthority])>,
    },
}

impl<'a> From<&'a ChainInformationFinality> for ChainInformationFinalityRef<'a> {
    fn from(finality: &'a ChainInformationFinality) -> ChainInformationFinalityRef<'a> {
        match finality {
            ChainInformationFinality::Outsourced => ChainInformationFinalityRef::Outsourced,
            ChainInformationFinality::Grandpa {
                finalized_triggered_authorities,
                after_finalized_block_authorities_set_id,
                finalized_scheduled_change,
            } => ChainInformationFinalityRef::Grandpa {
                after_finalized_block_authorities_set_id: *after_finalized_block_authorities_set_id,
                finalized_triggered_authorities,
                finalized_scheduled_change: finalized_scheduled_change
                    .as_ref()
                    .map(|(n, l)| (*n, &l[..])),
            },
        }
    }
}

/// Error when turning a [`ChainInformation`] into a [`ValidChainInformation`].
#[derive(Debug, derive_more::Display)]
pub enum ValidityError {
    /// The finalized block doesn't use the same consensus algorithm as the one in the chain
    /// information.
    ConsensusAlgorithmMismatch,
    /// Found a Babe slot start number for future Babe epoch number 0. A future Babe epoch 0 has
    /// no known starting slot.
    UnexpectedBabeSlotStartNumber,
    /// Missing Babe slot start number for Babe epoch number other than future epoch 0.
    MissingBabeSlotStartNumber,
    /// Finalized block is block number 0, and a Babe epoch information has been provided. This
    /// would imply the existence of a block -1 and below.
    UnexpectedBabeFinalizedEpoch,
    /// Finalized block is not number 0, but no Babe epoch information has been provided.
    NoBabeFinalizedEpoch,
    /// The slot of the finalized block is inferior to the start slot of the epoch it belongs to.
    HeaderBabeSlotInferiorToEpochStartSlot,
    /// Mismatch between the finalized block header digest and the Babe next epoch information.
    BabeEpochInfoMismatch,
    /// Scheduled GrandPa authorities change is before finalized block.
    ScheduledGrandPaChangeBeforeFinalized,
    /// The finalized block is block number 0, but the GrandPa authorities set id is not 0.
    FinalizedZeroButNonZeroAuthoritiesSetId,
    /// Error in a Babe epoch information.
    #[display(fmt = "Error in a Babe epoch information: {_0}")]
    InvalidBabe(BabeValidityError),
}

/// Error when checking the validity of a Babe epoch.
#[derive(Debug, derive_more::Display)]
pub enum BabeValidityError {
    /// Babe constant should be a fraction where the numerator is inferior or equal to the
    /// denominator.
    InvalidConstant,
}
