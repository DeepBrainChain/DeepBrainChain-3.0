#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use core::cmp::PartialEq;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode};
use sp_debug_derive::RuntimeDebug;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, DecodeWithMemTracking, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum VerifyErr {
    NotInBookList,
    TimeNotAllow,
    AlreadySubmitHash,
    AlreadySubmitRaw,
    NotSubmitHash,
    Overflow,
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, DecodeWithMemTracking, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum OnlineErr {
    ClaimRewardFailed,
    NotAllowedChangeMachineInfo,
    NotMachineController,
    CalcStakeAmountFailed,
    TelecomIsNull,
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, DecodeWithMemTracking, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum ReportErr {
    OrderNotAllowBook,
    AlreadyBooked,
    NotNeedEncryptedInfo,
    NotOrderReporter,
    OrderStatusNotFeat,
    NotOrderCommittee,
    NotInBookedList,
    NotProperCommittee,
}
