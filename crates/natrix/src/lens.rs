use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

pub trait Shape {
    type Read<'read>;
    type Mut<'write>;
}

impl<'a, T: ?Sized + 'static> Shape for &'a T {
    type Read<'read> = &'read T;
    type Mut<'write> = &'write mut T;
}

impl<T: Shape> Shape for Option<T> {
    type Read<'read> = Option<T::Read<'read>>;
    type Mut<'write> = Option<T::Mut<'write>>;
}

impl<O: Shape, E: Shape> Shape for Result<O, E> {
    type Read<'read> = Result<O::Read<'read>, E::Read<'read>>;
    type Mut<'write> = Result<O::Mut<'write>, E::Mut<'write>>;
}

#[must_use = "Lenses do nothing if not called"]
pub trait Optic<S: Shape>: Sized {
    type Output: Shape;

    #[must_use]
    fn get<'read>(&self, value: S::Read<'read>) -> <Self::Output as Shape>::Read<'read>;

    #[must_use]
    fn get_mut<'write>(&self, value: S::Mut<'write>) -> <Self::Output as Shape>::Mut<'write>;

    #[inline]
    fn then<R>(self, next: R) -> impl Optic<S, Output = R::Output>
    where
        R: Optic<Self::Output>,
    {
        Chain(self, next)
    }
}

#[must_use = "Lenses do nothing if not called"]
struct Chain<L, R>(L, R);

impl<S, L, R> Optic<S> for Chain<L, R>
where
    S: Shape,
    L: Optic<S>,
    R: Optic<L::Output>,
{
    type Output = R::Output;

    #[inline]
    fn get<'read>(&self, value: <S as Shape>::Read<'read>) -> <Self::Output as Shape>::Read<'read> {
        self.1.get(self.0.get(value))
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <S as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        self.1.get_mut(self.0.get_mut(value))
    }
}

struct Root<T>(PhantomData<T>);

impl<T: Shape> Optic<T> for Root<T> {
    type Output = T;

    #[inline]
    fn get<'read>(&self, value: <T as Shape>::Read<'read>) -> <Self::Output as Shape>::Read<'read> {
        value
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <T as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        value
    }
}

pub fn root<T: Shape>() -> impl Optic<T, Output = T> {
    Root::<T>(PhantomData)
}

struct DerefOptic;

impl<T: Deref + DerefMut + 'static> Optic<&T> for DerefOptic {
    type Output = &'static T::Target;

    #[inline]
    fn get<'read>(
        &self,
        value: <&T as Shape>::Read<'read>,
    ) -> <Self::Output as Shape>::Read<'read> {
        &**value
    }

    #[inline]
    fn get_mut<'write>(
        &self,
        value: <&T as Shape>::Mut<'write>,
    ) -> <Self::Output as Shape>::Mut<'write> {
        &mut **value
    }
}

pub trait ReferenceOptic<S: Shape>: Optic<S, Output = &'static Self::Target> {
    type Target: 'static;

    fn deref(self) -> impl Optic<S, Output = &'static <Self::Target as Deref>::Target>
    where
        Self::Target: Deref + DerefMut,
    {
        self.then(DerefOptic)
    }
}

impl<S, T, O> ReferenceOptic<S> for O
where
    S: Shape,
    T: 'static,
    O: Optic<S, Output = &'static T>,
{
    type Target = T;
}

#[cfg(test)]
mod tests {
    use super::{Optic, ReferenceOptic};

    fn assert_optic<S: super::Shape, T: super::Shape>(_: &impl Optic<S, Output = T>) {}

    #[test]
    fn root_works() {
        let lens = super::root::<&i32>();
        assert_optic::<&i32, &i32>(&lens);

        {
            let mut value = 0;
            assert_eq!(lens.get(&value), &0);
            *lens.get_mut(&mut value) = 10;
            assert_eq!(lens.get(&value), &10);
        }

        {
            let mut value = 0;
            assert_eq!(lens.get(&value), &0);
            *lens.get_mut(&mut value) = 10;
            assert_eq!(lens.get(&value), &10);
        }
    }

    #[test]
    fn deref_works() {
        let lens = super::root::<&String>().deref();
        assert_optic::<&String, &str>(&lens);

        let mut value = String::from("COOL");
        assert_eq!(lens.get(&value), "COOL");
        lens.get_mut(&mut value).make_ascii_lowercase();
        assert_eq!(lens.get(&value), "cool");
    }
}
