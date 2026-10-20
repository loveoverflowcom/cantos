//! Distinct, nonzero 128-bit identity/request namespaces. The shell generates random IDs.
//! Constructing an ID establishes neither its provenance nor authentication.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NilIdentifier;

impl fmt::Display for NilIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identity identifier must be nonzero")
    }
}

impl std::error::Error for NilIdentifier {}

/// Immutable Identity subject, distinct from the Server's ActorId.
///
/// ```compile_fail
/// use cantos_identity::domain::ids::SubjectId;
/// let forged = SubjectId(0);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubjectId(u128);

impl SubjectId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, NilIdentifier> {
        let value = u128::from_be_bytes(bytes);
        if value == 0 {
            return Err(NilIdentifier);
        }
        Ok(Self(value))
    }

    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

/// Database incarnation. The owner must generate a fresh value on create/restore.
///
/// ```compile_fail
/// use cantos_identity::domain::ids::{InstanceEpoch, SubjectId};
/// let subject = SubjectId::from_bytes([1; 16]).unwrap();
/// let epoch: InstanceEpoch = subject;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceEpoch(u128);

impl InstanceEpoch {
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, NilIdentifier> {
        let value = u128::from_be_bytes(bytes);
        if value == 0 {
            return Err(NilIdentifier);
        }
        Ok(Self(value))
    }

    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

/// A single authentication operation; never a reusable session or secret capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(u128);

impl OperationId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, NilIdentifier> {
        let value = u128::from_be_bytes(bytes);
        if value == 0 {
            return Err(NilIdentifier);
        }
        Ok(Self(value))
    }

    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::{InstanceEpoch, NilIdentifier, OperationId, SubjectId};

    #[test]
    fn nil_is_rejected_in_every_namespace() {
        assert_eq!(SubjectId::from_bytes([0; 16]), Err(NilIdentifier));
        assert_eq!(InstanceEpoch::from_bytes([0; 16]), Err(NilIdentifier));
        assert_eq!(OperationId::from_bytes([0; 16]), Err(NilIdentifier));
    }

    #[test]
    fn each_nonzero_bit_and_maximum_round_trip_without_truncation() {
        for bit in 0..128 {
            let bytes = (1_u128 << bit).to_be_bytes();
            assert_eq!(SubjectId::from_bytes(bytes).unwrap().to_bytes(), bytes);
            assert_eq!(InstanceEpoch::from_bytes(bytes).unwrap().to_bytes(), bytes);
            assert_eq!(OperationId::from_bytes(bytes).unwrap().to_bytes(), bytes);
        }
        let bytes = [255; 16];
        assert_eq!(SubjectId::from_bytes(bytes).unwrap().to_bytes(), bytes);
        assert_eq!(InstanceEpoch::from_bytes(bytes).unwrap().to_bytes(), bytes);
        assert_eq!(OperationId::from_bytes(bytes).unwrap().to_bytes(), bytes);
    }
}
