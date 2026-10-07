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

/// Call the given macro with every numeric type, and whether its a
/// integer or float.
macro_rules! numerics {
    ($macro:ident) => {
        $macro!(u8, Integer);
        $macro!(u16, Integer);
        $macro!(u32, Integer);
        $macro!(u64, Integer);
        $macro!(u128, Integer);
        $macro!(usize, Integer);
        $macro!(i8, Integer);
        $macro!(i16, Integer);
        $macro!(i32, Integer);
        $macro!(i64, Integer);
        $macro!(i128, Integer);
        $macro!(isize, Integer);
        $macro!(f32, Float);
        $macro!(f64, Float);
    };
}

pub(crate) use numerics;
pub(crate) use strings;
