//! Which stream this is.

/// Names one connection the server writes to more than once, so a push reaches
/// the phone it was decided for.
///
/// Minted at the edge and only ever relayed by the machine, the same way a
/// [`RequestId`](crate::RequestId) is — so a replay lands where it did
/// whichever numbers were drawn.
///
/// Not a `RequestId`: that names something owed exactly one answer and gone
/// once it has been given, and this one outlives its own request.
pub type ConnId = u64;
