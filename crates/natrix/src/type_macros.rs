//! Macros for implementing a trait on specific kinds of types.

/// Call the given macro with every string type, but converted to a `Cow`
macro_rules! strings {
    ($macro:ident) => {
        $macro!(&'static str, |this| ::std::borrow::Cow::Borrowed(this));
        $macro!(::std::string::String, |this| ::std::borrow::Cow::Owned(
            this
        ));
        $macro!(::std::borrow::Cow<'static, str>, |this| this);
        $macro!(::std::rc::Rc<str>, |this: ::std::rc::Rc<str>| {
            ::std::borrow::Cow::from(String::from(&*this))
        });
        $macro!(::std::sync::Arc<str>, |this: ::std::sync::Arc<str>| {
            ::std::borrow::Cow::from(String::from(&*this))
        });
        $macro!(::std::boxed::Box<str>, |this: ::std::boxed::Box<str>| {
            ::std::borrow::Cow::Owned(String::from(this))
        });
    };
}

/// Call the given macro with every numeric type, the formatting crate to use, and whether its a
/// integer or float.
macro_rules! numerics {
    ($macro:ident) => {
        $macro!(u8, itoa, Integer);
        $macro!(u16, itoa, Integer);
        $macro!(u32, itoa, Integer);
        $macro!(u64, itoa, Integer);
        $macro!(u128, itoa, Integer);
        $macro!(usize, itoa, Integer);
        $macro!(i8, itoa, Integer);
        $macro!(i16, itoa, Integer);
        $macro!(i32, itoa, Integer);
        $macro!(i64, itoa, Integer);
        $macro!(i128, itoa, Integer);
        $macro!(isize, itoa, Integer);
        $macro!(f32, ryu, Float);
        $macro!(f64, ryu, Float);
    };
}

pub(crate) use numerics;
pub(crate) use strings;
