use std::num::{NonZeroI32, NonZeroU32, TryFromIntError};

// Non-zero, positive integer below `1<<31`: [1..i32::MAX].
#[derive(Copy, Clone, Debug)]
pub struct PositiveU31 {
    value: u32,
}

impl TryFrom<u32> for PositiveU31 {
    type Error = TryFromIntError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        NonZeroI32::try_from(NonZeroU32::try_from(value)?)?;
        Ok(Self { value })
    }
}

impl From<PositiveU31> for u32 {
    fn from(value: PositiveU31) -> Self {
        value.value
    }
}

impl From<PositiveU31> for i32 {
    fn from(value: PositiveU31) -> Self {
        value.value as i32
    }
}

impl From<PositiveU31> for NonZeroU32 {
    fn from(value: PositiveU31) -> Self {
        Self::new(value.value).unwrap()
    }
}

impl From<PositiveU31> for NonZeroI32 {
    fn from(value: PositiveU31) -> Self {
        Self::new(value.value as i32).unwrap()
    }
}
