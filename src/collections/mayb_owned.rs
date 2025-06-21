use std::borrow::Borrow;
use std::ops::Deref;

pub enum MeMaybeOwned<'a, B, O>
where
    B: ?Sized,
    O: Borrow<B>,
{
    Borrowed(&'a B),
    Owned(O),
}

impl<'a, B, O> Deref for MeMaybeOwned<'a, B, O>
where
    B: ?Sized,
    O: Borrow<B>,
{
    type Target = B;

    fn deref(&self) -> &Self::Target {
        match self {
            MeMaybeOwned::Borrowed(b) => b,
            MeMaybeOwned::Owned(o) => o.borrow(),
        }
    }
}
