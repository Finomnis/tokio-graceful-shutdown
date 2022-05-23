use std::{
    error::Error,
    fmt::{Debug, Display},
    ops::Deref,
};

pub struct Direct<T: Error + Send + Sync>(pub T);

pub struct Wrapped<T: Deref + Send + Sync + Debug + Display>(pub T)
where
    <T as Deref>::Target: std::error::Error;

pub trait ErrorHolder: Error + Send + Sync + Sized + 'static {
    type Internal: Send + Sync;
    fn get_error(&self) -> &Self::Internal;
    fn into_error(self) -> Self::Internal;
    fn from_error(err: Self::Internal) -> Self;
}

impl<T: Error + Send + Sync + 'static> ErrorHolder for Direct<T> {
    type Internal = T;

    fn get_error(&self) -> &Self::Internal {
        &self.0
    }

    fn into_error(self) -> Self::Internal {
        self.0
    }

    fn from_error(err: Self::Internal) -> Self {
        Self(err)
    }
}

impl<T: Error + Send + Sync> Error for Direct<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

impl<T: Error + Send + Sync> Display for Direct<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl<T: Error + Send + Sync> Debug for Direct<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T: Deref + Send + Sync + Debug + Display + 'static> ErrorHolder for Wrapped<T>
where
    <T as Deref>::Target: std::error::Error,
{
    type Internal = T;

    fn get_error(&self) -> &Self::Internal {
        &self.0
    }

    fn into_error(self) -> Self::Internal {
        self.0
    }

    fn from_error(err: Self::Internal) -> Self {
        Self(err)
    }
}

impl<T: Deref + Send + Sync + Debug + Display> Error for Wrapped<T>
where
    <T as Deref>::Target: std::error::Error,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

impl<T: Deref + Send + Sync + Debug + Display> Display for Wrapped<T>
where
    <T as Deref>::Target: std::error::Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
impl<T: Deref + Send + Sync + Debug + Display> Debug for Wrapped<T>
where
    <T as Deref>::Target: std::error::Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}
